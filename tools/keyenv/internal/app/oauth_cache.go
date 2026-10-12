package app

import (
	"encoding/json"
	"errors"
	"os"
	"path/filepath"

	"github.com/miles1150/personal-plugins/tools/keyenv/internal/drive"
	"github.com/miles1150/personal-plugins/tools/keyenv/internal/vault"
	"golang.org/x/oauth2"
)

const OAuthCacheFile = "oauth-login.json"

type loginRecord struct {
	Version    int          `json:"version"`
	VaultID    string       `json:"vault_id"`
	Device     string       `json:"device"`
	HeaderHash string       `json:"header_sha256"`
	Account    string       `json:"account"`
	ClientID   string       `json:"client_id"`
	Scope      string       `json:"scope"`
	Token      oauth2.Token `json:"token"`
}

// LoginCache is device-local and independent of the ciphertext Bundle/backup.
// Callers hold the Session lock for the lifetime of the OAuth client.
type LoginCache struct {
	session           *Session
	account, clientID string
}

func (s *Session) LoginCache(account, clientID string) *LoginCache {
	return &LoginCache{session: s, account: account, clientID: clientID}
}
func (c *LoginCache) objectID() string { return "oauth-login-" + c.session.Binding.Device }
func (c *LoginCache) Load() (*oauth2.Token, error) {
	s := c.session
	path := filepath.Join(s.Home, OAuthCacheFile)
	if _, err := os.Lstat(path); errors.Is(err, os.ErrNotExist) {
		return nil, nil
	}
	data, err := readPrivateLimit(path, 65536)
	if err != nil {
		return nil, err
	}
	plain, err := vault.Open(s.Key, s.Header.VaultID, c.objectID(), data)
	if err != nil {
		return nil, err
	}
	defer clear(plain)
	var r loginRecord
	if err = decode(plain, &r); err != nil {
		return nil, err
	}
	if r.Version != 1 || r.VaultID != s.Header.VaultID || r.Device != s.Binding.Device || r.HeaderHash != headerHash(s.Bundle.Header) || r.Account != c.account || r.ClientID != c.clientID || r.Scope != drive.DriveFileScope || !drive.ValidCachedToken(&r.Token) {
		return nil, vault.ErrIntegrity
	}
	return &r.Token, nil
}
func (c *LoginCache) Save(token *oauth2.Token) error {
	s := c.session
	if !drive.ValidCachedToken(token) || c.account == "" || c.clientID == "" || privateDir(s.Home) != nil {
		return vault.ErrInvalid
	}
	r := loginRecord{Version: 1, VaultID: s.Header.VaultID, Device: s.Binding.Device, HeaderHash: headerHash(s.Bundle.Header), Account: c.account, ClientID: c.clientID, Scope: drive.DriveFileScope, Token: *token}
	plain, err := json.Marshal(r)
	if err != nil {
		return vault.ErrInvalid
	}
	defer clear(plain)
	data, err := vault.Seal(s.Key, s.Header.VaultID, c.objectID(), plain)
	if err != nil {
		return err
	}
	path := filepath.Join(s.Home, OAuthCacheFile)
	if info, err := os.Lstat(path); err == nil {
		if !info.Mode().IsRegular() || info.Mode().Perm()&0077 != 0 {
			return ErrStorage
		}
	} else if !errors.Is(err, os.ErrNotExist) {
		return ErrStorage
	}
	f, err := os.CreateTemp(s.Home, ".oauth-login-*")
	if err != nil {
		return ErrStorage
	}
	defer f.Close()
	defer os.Remove(f.Name())
	if f.Chmod(0600) != nil {
		return ErrStorage
	}
	if _, err = f.Write(data); err != nil || f.Sync() != nil || f.Close() != nil {
		return ErrStorage
	}
	if os.Rename(f.Name(), path) != nil || syncDir(s.Home) != nil {
		return ErrStorage
	}
	return nil
}

// OAuthCacheState validates local binding only; it does not prove a remote grant
// remains valid, nor expose any token material through status output.
func (s *Session) OAuthCacheState() string {
	token, err := s.LoginCache(s.Binding.Account, s.Binding.ClientID).Load()
	if err != nil {
		return "invalid"
	}
	if token == nil {
		return "absent"
	}
	return "available"
}

// ForgetOAuth deletes only local encrypted login material, not the remote grant.
func (s *Session) ForgetOAuth() error {
	path := filepath.Join(s.Home, OAuthCacheFile)
	info, err := os.Lstat(path)
	if errors.Is(err, os.ErrNotExist) {
		return nil
	}
	if err != nil || !info.Mode().IsRegular() {
		return ErrStorage
	}
	if os.Remove(path) != nil || syncDir(s.Home) != nil {
		return ErrStorage
	}
	return nil
}
