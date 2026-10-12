package app

import (
	"bytes"
	"encoding/json"
	"errors"
	"os"
	"path/filepath"
	"testing"

	"github.com/miles1150/personal-plugins/tools/keyenv/internal/vault"
)

const syntheticClientSecret = "SYNTHETIC-OAuth-client-secret-only"

func clientBundleFixture(t *testing.T) (*Session, string, string) {
	t.Helper()
	s := create(t)
	s.Binding.ClientID = "synthetic.apps.googleusercontent.com"
	p := PublicOAuthClient{ClientID: s.Binding.ClientID, ProjectID: "synthetic-project", AuthURI: "https://accounts.google.com/o/oauth2/auth", TokenURI: "https://oauth2.googleapis.com/token", CertURI: "https://www.googleapis.com/oauth2/v1/certs", RedirectURIs: []string{"http://localhost"}}
	data, _ := json.Marshal(struct {
		Installed privateOAuthClient `json:"installed"`
	}{Installed: privateOAuthClient{PublicOAuthClient: p, Secret: syntheticClientSecret}})
	source := filepath.Join(filepath.Dir(s.Home), "source-client.json")
	if err := os.WriteFile(source, data, 0600); err != nil {
		t.Fatal(err)
	}
	return s, source, filepath.Join(filepath.Dir(s.Home), "bundle")
}
func TestClientBundlePublicAllowlistEncryptedSecretAndPasswordRestore(t *testing.T) {
	s, source, directory := clientBundleFixture(t)
	before, _ := os.ReadFile(source)
	if err := s.PackOAuthClient(source, directory); err != nil {
		t.Fatal(err)
	}
	public, _ := os.ReadFile(filepath.Join(directory, ClientPublicFile))
	sealed, _ := os.ReadFile(filepath.Join(directory, ClientSecretFile))
	if bytes.Contains(public, []byte("client_secret")) || bytes.Contains(public, []byte(syntheticClientSecret)) || bytes.Contains(sealed, []byte(syntheticClientSecret)) || bytes.Contains(sealed, testPassword) {
		t.Fatal("plaintext credential published")
	}
	if !bytes.Contains(public, []byte(s.Binding.ClientID)) || !bytes.Contains(public, []byte("synthetic-project")) {
		t.Fatal("public metadata missing")
	}
	after, _ := os.ReadFile(source)
	if !bytes.Equal(before, after) {
		t.Fatal("original client changed")
	}
	b, err := ReadOAuthClientBundle(directory)
	if err != nil {
		t.Fatal(err)
	}
	if !bytes.Equal(b.Header(), s.Bundle.Header) {
		t.Fatal("wrapped header changed")
	}
	output := filepath.Join(filepath.Dir(s.Home), "new-machine", "client.json")
	if err = b.RestoreClient(output, []byte("wrong-password")); !errors.Is(err, vault.ErrAuthentication) {
		t.Fatal("wrong password accepted", err)
	}
	if _, err = os.Stat(output); !os.IsNotExist(err) {
		t.Fatal("plaintext created before unlock")
	}
	if err = b.RestoreClient(output, testPassword); err != nil {
		t.Fatal(err)
	}
	full, _ := os.ReadFile(output)
	var restored struct {
		Installed privateOAuthClient `json:"installed"`
	}
	if decode(full, &restored) != nil || restored.Installed.Secret != syntheticClientSecret || restored.Installed.ClientID != s.Binding.ClientID {
		t.Fatal("client reconstruction changed identity")
	}
	info, _ := os.Stat(output)
	parent, _ := os.Stat(filepath.Dir(output))
	if info.Mode().Perm() != 0600 || parent.Mode().Perm() != 0700 {
		t.Fatal("private client public")
	}
	if err = b.RestoreClient(output, testPassword); err != nil {
		t.Fatal("identical restore not idempotent")
	}
	if err = s.PackOAuthClient(source, directory); !errors.Is(err, ErrExists) {
		t.Fatal("bundle overwritten")
	}
}
func TestClientBundleRejectsUnknownSecretFieldsForeignIdentityAndEndpoints(t *testing.T) {
	for _, kind := range []string{"unknown-secret", "root-token", "foreign-client", "auth-origin", "token-origin", "unsafe-redirect", "public-source", "symlink-source"} {
		t.Run(kind, func(t *testing.T) {
			s, source, directory := clientBundleFixture(t)
			data, _ := os.ReadFile(source)
			var raw map[string]any
			json.Unmarshal(data, &raw)
			c := raw["installed"].(map[string]any)
			switch kind {
			case "unknown-secret":
				c["refresh_token"] = "SYNTHETIC-refresh"
			case "root-token":
				raw["access_token"] = "SYNTHETIC-access"
			case "foreign-client":
				c["client_id"] = "foreign.apps.googleusercontent.com"
			case "auth-origin":
				c["auth_uri"] = "https://foreign.invalid/auth"
			case "token-origin":
				c["token_uri"] = "https://foreign.invalid/token"
			case "unsafe-redirect":
				c["redirect_uris"] = []string{"https://foreign.invalid/callback"}
			}
			data, _ = json.Marshal(raw)
			os.WriteFile(source, data, 0600)
			if kind == "public-source" {
				os.Chmod(source, 0644)
			}
			if kind == "symlink-source" {
				link := filepath.Join(filepath.Dir(source), "link.json")
				os.Symlink(source, link)
				source = link
			}
			if err := s.PackOAuthClient(source, directory); err == nil {
				t.Fatal("unsafe source published")
			}
			if _, err := os.Stat(directory); !os.IsNotExist(err) {
				t.Fatal("rejected source produced artifacts")
			}
		})
	}
}
func TestClientBundleRejectsTamperingAndWrongPair(t *testing.T) {
	for _, kind := range []string{"public", "public-and-hash", "encrypted", "header", "unknown-field", "swapped-pair"} {
		t.Run(kind, func(t *testing.T) {
			s, source, directory := clientBundleFixture(t)
			if err := s.PackOAuthClient(source, directory); err != nil {
				t.Fatal(err)
			}
			switch kind {
			case "public", "public-and-hash":
				p, _ := readBundleJSON(filepath.Join(directory, ClientPublicFile))
				var pub publicClientBundle
				decode(p, &pub)
				pub.Installed.ProjectID = "changed-project"
				p, _ = json.Marshal(pub)
				os.WriteFile(filepath.Join(directory, ClientPublicFile), p, 0600)
				if kind == "public-and-hash" {
					e, _ := readBundleJSON(filepath.Join(directory, ClientSecretFile))
					var enc encryptedClientBundle
					decode(e, &enc)
					enc.PublicHash, _ = publicClientHash(pub)
					e, _ = json.Marshal(enc)
					os.WriteFile(filepath.Join(directory, ClientSecretFile), e, 0600)
				}
			case "encrypted", "header":
				e, _ := readBundleJSON(filepath.Join(directory, ClientSecretFile))
				var enc encryptedClientBundle
				decode(e, &enc)
				if kind == "encrypted" {
					enc.Encrypted = []byte(`{"version":1,"nonce":"AAAAAAAAAAAAAAAA","ciphertext":"AAAA"}`)
				} else {
					h, key, err := vault.Create(testPassword, vault.DefaultKDF())
					if err != nil {
						t.Fatal(err)
					}
					clear(key)
					enc.Header, _ = json.Marshal(h)
				}
				e, _ = json.Marshal(enc)
				os.WriteFile(filepath.Join(directory, ClientSecretFile), e, 0600)
			case "unknown-field":
				p, _ := readBundleJSON(filepath.Join(directory, ClientPublicFile))
				var pub map[string]any
				json.Unmarshal(p, &pub)
				pub["client_secret"] = "unexpected"
				p, _ = json.Marshal(pub)
				os.WriteFile(filepath.Join(directory, ClientPublicFile), p, 0600)
			case "swapped-pair":
				other, src, dir := clientBundleFixture(t)
				other.Binding.ClientID = "another.apps.googleusercontent.com"
				data, _ := os.ReadFile(src)
				data = bytes.ReplaceAll(data, []byte("synthetic.apps.googleusercontent.com"), []byte(other.Binding.ClientID))
				os.WriteFile(src, data, 0600)
				if err := other.PackOAuthClient(src, dir); err != nil {
					t.Fatal(err)
				}
				pub, _ := os.ReadFile(filepath.Join(dir, ClientPublicFile))
				os.WriteFile(filepath.Join(directory, ClientPublicFile), pub, 0600)
			}
			b, err := ReadOAuthClientBundle(directory)
			out := filepath.Join(filepath.Dir(s.Home), "restored", "client.json")
			if err == nil {
				err = b.RestoreClient(out, testPassword)
			}
			if err == nil {
				t.Fatal("tampered bootstrap accepted")
			}
			if _, err = os.Stat(out); !os.IsNotExist(err) {
				t.Fatal("tampered bootstrap installed plaintext")
			}
		})
	}
}
func TestClientBundleRestoreRefusesGitSymlinkAndForeignExistingClient(t *testing.T) {
	s, src, dir := clientBundleFixture(t)
	if err := s.PackOAuthClient(src, dir); err != nil {
		t.Fatal(err)
	}
	b, err := ReadOAuthClientBundle(dir)
	if err != nil {
		t.Fatal(err)
	}
	root := t.TempDir()
	os.Chmod(root, 0700)
	os.Mkdir(filepath.Join(root, ".git"), 0700)
	if err = b.RestoreClient(filepath.Join(root, "client.json"), testPassword); !errors.Is(err, vault.ErrInvalid) {
		t.Fatal("plaintext installed in git", err)
	}
	sub := filepath.Join(root, "subdir")
	os.Mkdir(sub, 0700)
	alias := filepath.Join(t.TempDir(), "checkout-subdir")
	os.Symlink(sub, alias)
	if err = b.RestoreClient(filepath.Join(alias, "oauth", "client.json"), testPassword); !errors.Is(err, vault.ErrInvalid) {
		t.Fatal("git ancestor alias bypassed", err)
	}
	other := t.TempDir()
	os.Chmod(other, 0700)
	target := filepath.Join(other, "original")
	os.WriteFile(target, []byte("preserve"), 0600)
	link := filepath.Join(other, "client.json")
	os.Symlink(target, link)
	if err = b.RestoreClient(link, testPassword); !errors.Is(err, ErrStorage) {
		t.Fatal("symlink overwritten", err)
	}
	data, _ := os.ReadFile(target)
	if string(data) != "preserve" {
		t.Fatal("symlink target changed")
	}
	os.Remove(link)
	os.WriteFile(link, []byte(`{"installed":{"client_id":"foreign","client_secret":"SYNTHETIC-foreign"}}`), 0600)
	before, _ := os.ReadFile(link)
	if err = b.RestoreClient(link, testPassword); !errors.Is(err, ErrExists) {
		t.Fatal("foreign client overwritten", err)
	}
	after, _ := os.ReadFile(link)
	if !bytes.Equal(before, after) {
		t.Fatal("foreign client changed")
	}
}
