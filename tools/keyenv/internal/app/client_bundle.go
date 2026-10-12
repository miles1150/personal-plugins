package app

import (
	"bytes"
	"crypto/sha256"
	"encoding/hex"
	"encoding/json"
	"errors"
	"net/url"
	"os"
	"path/filepath"
	"strings"

	"github.com/miles1150/personal-plugins/tools/keyenv/internal/vault"
)

const ClientPublicFile = "client.public.json"
const ClientSecretFile = "client.secret.enc.json"

// This allowlist deliberately excludes every token and secret. Unknown fields
// are rejected instead of being copied to the public configuration.
type PublicOAuthClient struct {
	ClientID     string   `json:"client_id"`
	ProjectID    string   `json:"project_id,omitempty"`
	AuthURI      string   `json:"auth_uri,omitempty"`
	TokenURI     string   `json:"token_uri,omitempty"`
	CertURI      string   `json:"auth_provider_x509_cert_url,omitempty"`
	RedirectURIs []string `json:"redirect_uris,omitempty"`
}
type publicClientBundle struct {
	Version   int               `json:"version"`
	Installed PublicOAuthClient `json:"installed"`
}
type privateOAuthClient struct {
	PublicOAuthClient
	Secret string `json:"client_secret"`
}
type encryptedClientBundle struct {
	Version    int    `json:"version"`
	PublicHash string `json:"public_sha256"`
	// Original wrapped-key header bytes; contains no unwrapped key or password.
	Header    []byte          `json:"wrapped_key_header"`
	Encrypted json.RawMessage `json:"encrypted_secret"`
}
type clientSecretPayload struct {
	Version    int    `json:"version"`
	PublicHash string `json:"public_sha256"`
	HeaderHash string `json:"header_sha256"`
	ClientID   string `json:"client_id"`
	Secret     string `json:"client_secret"`
}
type OAuthClientBundle struct {
	public publicClientBundle
	sealed encryptedClientBundle
}

func validPublicClient(c PublicOAuthClient) bool {
	if c.ClientID == "" || len(c.ClientID) > 512 || strings.ContainsAny(c.ClientID, " \r\n\x00\t") || len(c.ProjectID) > 128 || strings.ContainsAny(c.ProjectID, "\r\n\x00") {
		return false
	}
	if c.AuthURI != "" && c.AuthURI != "https://accounts.google.com/o/oauth2/auth" && c.AuthURI != "https://accounts.google.com/o/oauth2/v2/auth" {
		return false
	}
	if c.TokenURI != "" && c.TokenURI != "https://oauth2.googleapis.com/token" && c.TokenURI != "https://accounts.google.com/o/oauth2/token" {
		return false
	}
	if c.CertURI != "" && c.CertURI != "https://www.googleapis.com/oauth2/v1/certs" && c.CertURI != "https://www.googleapis.com/oauth2/v3/certs" {
		return false
	}
	if len(c.RedirectURIs) > 8 {
		return false
	}
	for _, v := range c.RedirectURIs {
		u, err := url.Parse(v)
		if err != nil || len(v) > 512 || u.Scheme != "http" || (u.Hostname() != "localhost" && u.Hostname() != "127.0.0.1" && u.Hostname() != "::1") || u.User != nil || u.RawQuery != "" || u.Fragment != "" {
			return false
		}
	}
	return true
}
func validClientSecret(v string) bool {
	return v != "" && len(v) <= 8192 && !strings.ContainsAny(v, " \t\r\n\x00")
}
func publicClientHash(p publicClientBundle) (string, error) {
	data, err := json.Marshal(p)
	if err != nil {
		return "", vault.ErrInvalid
	}
	sum := sha256.Sum256(data)
	return hex.EncodeToString(sum[:]), nil
}
func clientBundleObjectID(hash string) string { return "oauth-client-bootstrap-" + hash }

func (s *Session) PackOAuthClient(input, output string) error {
	if !filepath.IsAbs(input) || !filepath.IsAbs(output) || len(s.Key) != 32 {
		return vault.ErrInvalid
	}
	data, err := readPrivateLimit(input, 65536)
	if err != nil {
		return err
	}
	defer clear(data)
	var source struct {
		Installed privateOAuthClient `json:"installed"`
	}
	if err = decode(data, &source); err != nil {
		return err
	}
	c := source.Installed
	if !validPublicClient(c.PublicOAuthClient) || !validClientSecret(c.Secret) || c.ClientID != s.Binding.ClientID {
		return vault.ErrIntegrity
	}
	p := publicClientBundle{Version: 1, Installed: c.PublicOAuthClient}
	hash, err := publicClientHash(p)
	if err != nil {
		return err
	}
	plain, err := json.Marshal(clientSecretPayload{Version: 1, PublicHash: hash, HeaderHash: headerHash(s.Bundle.Header), ClientID: c.ClientID, Secret: c.Secret})
	if err != nil {
		return vault.ErrInvalid
	}
	defer clear(plain)
	sealed, err := vault.Seal(s.Key, s.Header.VaultID, clientBundleObjectID(hash), plain)
	if err != nil {
		return err
	}
	b := OAuthClientBundle{public: p, sealed: encryptedClientBundle{Version: 1, PublicHash: hash, Header: bytes.Clone(s.Bundle.Header), Encrypted: sealed}}
	checked, err := b.unlockKey(s.Key)
	if err != nil || checked.Secret != c.Secret {
		return vault.ErrIntegrity
	}
	publicData, err := json.MarshalIndent(p, "", "  ")
	if err != nil {
		return vault.ErrInvalid
	}
	secretData, err := json.MarshalIndent(b.sealed, "", "  ")
	if err != nil {
		return vault.ErrInvalid
	}
	if bytes.Contains(publicData, []byte(c.Secret)) || bytes.Contains(secretData, []byte(c.Secret)) {
		return vault.ErrIntegrity
	}
	if os.Mkdir(output, 0700) != nil {
		if _, e := os.Lstat(output); e == nil {
			return ErrExists
		}
		return ErrStorage
	}
	complete, publicWritten, secretWritten := false, false, false
	defer func() {
		if !complete {
			if publicWritten {
				os.Remove(filepath.Join(output, ClientPublicFile))
			}
			if secretWritten {
				os.Remove(filepath.Join(output, ClientSecretFile))
			}
			os.Remove(output)
		}
	}()
	if err = exclusive(filepath.Join(output, ClientPublicFile), append(publicData, '\n')); err != nil {
		return err
	}
	publicWritten = true
	if err = exclusive(filepath.Join(output, ClientSecretFile), append(secretData, '\n')); err != nil {
		return err
	}
	secretWritten = true
	if err = syncDir(output); err != nil {
		return err
	}
	complete = true
	return syncDir(filepath.Dir(output))
}

func readBundleJSON(path string) ([]byte, error) {
	info, err := os.Lstat(path)
	if err != nil || !info.Mode().IsRegular() || info.Size() > 65536 {
		return nil, ErrStorage
	}
	data, err := os.ReadFile(path)
	if err != nil || len(data) > 65536 {
		return nil, ErrStorage
	}
	return data, nil
}
func ReadOAuthClientBundle(directory string) (*OAuthClientBundle, error) {
	if !filepath.IsAbs(directory) {
		return nil, vault.ErrInvalid
	}
	info, err := os.Lstat(directory)
	if err != nil || !info.IsDir() {
		return nil, ErrStorage
	}
	p, err := readBundleJSON(filepath.Join(directory, ClientPublicFile))
	if err != nil {
		return nil, err
	}
	secret, err := readBundleJSON(filepath.Join(directory, ClientSecretFile))
	if err != nil {
		return nil, err
	}
	b := &OAuthClientBundle{}
	if err = decode(p, &b.public); err != nil {
		return nil, err
	}
	if err = decode(secret, &b.sealed); err != nil {
		return nil, err
	}
	hash, err := publicClientHash(b.public)
	if err != nil || b.public.Version != 1 || b.sealed.Version != 1 || !validPublicClient(b.public.Installed) || hash != b.sealed.PublicHash {
		return nil, vault.ErrIntegrity
	}
	if _, err = vault.ParseHeader(b.sealed.Header); err != nil {
		return nil, err
	}
	return b, nil
}
func (b *OAuthClientBundle) unlockKey(key []byte) (privateOAuthClient, error) {
	h, err := vault.ParseHeader(b.sealed.Header)
	if err != nil {
		return privateOAuthClient{}, err
	}
	plain, err := vault.Open(key, h.VaultID, clientBundleObjectID(b.sealed.PublicHash), b.sealed.Encrypted)
	if err != nil {
		return privateOAuthClient{}, err
	}
	defer clear(plain)
	var payload clientSecretPayload
	if err = decode(plain, &payload); err != nil {
		return privateOAuthClient{}, err
	}
	if payload.Version != 1 || payload.PublicHash != b.sealed.PublicHash || payload.HeaderHash != headerHash(b.sealed.Header) || payload.ClientID != b.public.Installed.ClientID || !validClientSecret(payload.Secret) {
		return privateOAuthClient{}, vault.ErrIntegrity
	}
	return privateOAuthClient{PublicOAuthClient: b.public.Installed, Secret: payload.Secret}, nil
}
func (b *OAuthClientBundle) Header() []byte { return bytes.Clone(b.sealed.Header) }
func (b *OAuthClientBundle) RestoreClient(output string, password []byte) error {
	_, key, err := vault.Unlock(b.sealed.Header, password)
	if err != nil {
		return err
	}
	defer clear(key)
	c, err := b.unlockKey(key)
	if err != nil {
		return err
	}
	data, err := json.Marshal(struct {
		Installed privateOAuthClient `json:"installed"`
	}{Installed: c})
	if err != nil {
		return vault.ErrInvalid
	}
	defer clear(data)
	return installPrivateClient(output, data)
}

func installPrivateClient(path string, data []byte) error {
	if !filepath.IsAbs(path) {
		return vault.ErrInvalid
	}
	// Plaintext credentials never go into a Git checkout, even with an explicitly
	// supplied output path. Ciphertext/bootstrap files are separately publishable.
	for dir := filepath.Dir(path); ; dir = filepath.Dir(dir) {
		if _, err := os.Lstat(filepath.Join(dir, ".git")); err == nil {
			return vault.ErrInvalid
		} else if !errors.Is(err, os.ErrNotExist) {
			return ErrStorage
		}
		if dir == filepath.Dir(dir) {
			break
		}
	}
	directory := filepath.Dir(path)
	// Resolve ancestor aliases before checking .git, so a symlink to a checkout
	// subdirectory cannot bypass the plaintext destination restriction.
	canonicalParent, err := filepath.EvalSymlinks(filepath.Dir(directory))
	if err != nil {
		return ErrStorage
	}
	for dir := filepath.Join(canonicalParent, filepath.Base(directory)); ; dir = filepath.Dir(dir) {
		if _, e := os.Lstat(filepath.Join(dir, ".git")); e == nil {
			return vault.ErrInvalid
		} else if !errors.Is(e, os.ErrNotExist) {
			return ErrStorage
		}
		if dir == filepath.Dir(dir) {
			break
		}
	}
	if err := os.Mkdir(directory, 0700); err != nil && !errors.Is(err, os.ErrExist) {
		return ErrStorage
	}
	if err := privateDir(directory); err != nil {
		return err
	}
	if _, err := os.Lstat(path); err == nil {
		old, err := readPrivateLimit(path, 65536)
		if err != nil {
			return err
		}
		defer clear(old)
		var previous, expected struct {
			Installed privateOAuthClient `json:"installed"`
		}
		if decode(old, &previous) != nil || decode(data, &expected) != nil || previous.Installed.ClientID != expected.Installed.ClientID || previous.Installed.Secret != expected.Installed.Secret {
			return ErrExists
		}
		oldHash, _ := publicClientHash(publicClientBundle{Version: 1, Installed: previous.Installed.PublicOAuthClient})
		newHash, _ := publicClientHash(publicClientBundle{Version: 1, Installed: expected.Installed.PublicOAuthClient})
		if oldHash != newHash {
			return ErrExists
		}
		return nil
	} else if !errors.Is(err, os.ErrNotExist) {
		return ErrStorage
	}
	return exclusive(path, data)
}
