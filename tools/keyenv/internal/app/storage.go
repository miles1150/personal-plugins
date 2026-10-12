package app

import (
	"bytes"
	"encoding/json"
	"errors"
	"io"
	"os"
	"path/filepath"
	"time"

	"github.com/miles1150/personal-plugins/tools/keyenv/internal/vault"
)

var ErrStorage = errors.New("LOCAL_STORAGE_FAILED")
var ErrExists = errors.New("ALREADY_EXISTS")
var ErrLocked = errors.New("VAULT_BUSY")

// Binding is encrypted and authenticated with the vault key. No OAuth token is saved.
type Binding struct {
	Device       string `json:"device"`
	Folder       string `json:"folder,omitempty"`
	HeaderID     string `json:"header_id,omitempty"`
	Account      string `json:"account,omitempty"`
	ClientID     string `json:"client_id,omitempty"`
	ClientPath   string `json:"client_path,omitempty"`
	SetupPending bool   `json:"setup_pending,omitempty"`
}
type Bundle struct {
	Version  int             `json:"version"`
	Header   json.RawMessage `json:"header"`
	Binding  []byte          `json:"binding"`
	State    vault.State     `json:"state"`
	LastPull time.Time       `json:"last_pull"`
}
type Session struct {
	Home    string
	Bundle  Bundle
	Header  vault.Header
	Binding Binding
	Key     []byte
	unlock  func()
}

func decode(data []byte, v any) error {
	if len(data) > 48*vault.MaxObject {
		return vault.ErrInvalid
	}
	d := json.NewDecoder(bytes.NewReader(data))
	d.DisallowUnknownFields()
	if d.Decode(v) != nil || d.Decode(new(any)) != io.EOF {
		return vault.ErrInvalid
	}
	return nil
}
func privateDir(path string) error {
	i, e := os.Lstat(path)
	if e != nil || !i.IsDir() || i.Mode().Perm()&0077 != 0 {
		return ErrStorage
	}
	return nil
}
func readPrivate(path string) ([]byte, error) {
	i, e := os.Lstat(path)
	if e != nil || !i.Mode().IsRegular() || i.Mode().Perm()&0077 != 0 || i.Size() > 48*vault.MaxObject {
		return nil, ErrStorage
	}
	f, e := os.Open(path)
	if e != nil {
		return nil, ErrStorage
	}
	defer f.Close()
	b, e := io.ReadAll(io.LimitReader(f, 48*vault.MaxObject+1))
	if e != nil || len(b) > 48*vault.MaxObject {
		return nil, ErrStorage
	}
	return b, nil
}

func exclusive(path string, data []byte) error {
	dir := filepath.Dir(path)
	f, e := os.CreateTemp(dir, ".keyenv-write-*")
	if e != nil {
		return ErrStorage
	}
	tmp := f.Name()
	defer os.Remove(tmp)
	defer f.Close()
	if f.Chmod(0600) != nil {
		return ErrStorage
	}
	if _, e = f.Write(data); e != nil {
		return ErrStorage
	}
	if e = f.Sync(); e != nil {
		return ErrStorage
	}
	if e = f.Close(); e != nil {
		return ErrStorage
	}
	// Hard link publishes a complete file atomically without replacing any target.
	if e = os.Link(tmp, path); e != nil {
		if errors.Is(e, os.ErrExist) {
			return ErrExists
		}
		return ErrStorage
	}
	return syncDir(dir)
}
func syncDir(path string) error {
	f, e := os.Open(path)
	if e != nil {
		return ErrStorage
	}
	defer f.Close()
	if f.Sync() != nil {
		return ErrStorage
	}
	return nil
}

func NewBundle(password []byte) (Bundle, vault.Header, []byte, error) {
	h, key, e := vault.Create(password, vault.DefaultKDF())
	if e != nil {
		return Bundle{}, h, nil, e
	}
	hb, e := json.Marshal(h)
	if e != nil {
		clear(key)
		return Bundle{}, h, nil, vault.ErrInvalid
	}
	b := Bundle{Version: 1, Header: hb, State: vault.State{Confirmed: map[string][]byte{}, Pending: map[string][]byte{}}}
	return b, h, key, nil
}
func sealBinding(b *Bundle, h vault.Header, key []byte, config Binding) error {
	plain, e := json.Marshal(config)
	if e != nil {
		return vault.ErrInvalid
	}
	defer clear(plain)
	b.Binding, e = vault.Seal(key, h.VaultID, h.VaultID, plain)
	return e
}

// Initialize creates only an empty local vault. It does not permit offline edits.
func Initialize(home string, password []byte) error {
	if !filepath.IsAbs(home) {
		return vault.ErrInvalid
	}
	if e := os.Mkdir(home, 0700); e != nil {
		if errors.Is(e, os.ErrExist) {
			return ErrExists
		}
		return ErrStorage
	}
	complete := false
	defer func() {
		if !complete {
			os.Remove(filepath.Join(home, "vault.json"))
			os.Remove(home)
		}
	}()
	b, h, key, e := NewBundle(password)
	if e != nil {
		return e
	}
	defer clear(key)
	if e = sealBinding(&b, h, key, Binding{Device: vault.NewID()}); e != nil {
		return e
	}
	data, e := json.Marshal(b)
	if e != nil {
		return vault.ErrInvalid
	}
	if e = exclusive(filepath.Join(home, "vault.json"), data); e != nil {
		return e
	}
	complete = true
	return syncDir(home)
}

func InitializeFromHeader(home string, header []byte, password []byte, binding Binding) error {
	if !filepath.IsAbs(home) {
		return vault.ErrInvalid
	}
	h, key, e := vault.Unlock(header, password)
	if e != nil {
		return e
	}
	defer clear(key)
	b := Bundle{Version: 1, Header: bytes.Clone(header), State: vault.State{Confirmed: map[string][]byte{}, Pending: map[string][]byte{}}}
	binding.Device = vault.NewID()
	if e = sealBinding(&b, h, key, binding); e != nil {
		return e
	}
	data, e := json.Marshal(b)
	if e != nil {
		return vault.ErrInvalid
	}
	if e = os.Mkdir(home, 0700); e != nil {
		if errors.Is(e, os.ErrExist) {
			return ErrExists
		}
		return ErrStorage
	}
	if e = exclusive(filepath.Join(home, "vault.json"), data); e != nil {
		_ = os.Remove(home) // removes only an empty directory created above
		return e
	}
	return syncDir(home)
}

func Open(home string, password []byte) (*Session, error) {
	if !filepath.IsAbs(home) || privateDir(home) != nil {
		return nil, ErrStorage
	}
	unlock, e := lock(home)
	if e != nil {
		return nil, e
	}
	failed := true
	defer func() {
		if failed {
			unlock()
		}
	}()
	b, e := readPrivate(filepath.Join(home, "vault.json"))
	if e != nil {
		return nil, e
	}
	var bundle Bundle
	if e = decode(b, &bundle); e != nil {
		return nil, e
	}
	if bundle.Version != 1 {
		return nil, vault.ErrUnsupported
	}
	h, key, e := vault.Unlock(bundle.Header, password)
	if e != nil {
		return nil, e
	}
	s := &Session{Home: home, Bundle: bundle, Header: h, Key: key, unlock: unlock}
	plain, e := vault.Open(key, h.VaultID, h.VaultID, bundle.Binding)
	if e != nil {
		clear(key)
		return nil, e
	}
	defer clear(plain)
	if e = decode(plain, &s.Binding); e != nil {
		clear(key)
		return nil, e
	}
	if !vault.ValidID(s.Binding.Device) || (s.Binding.Folder != "" && (!vault.ValidID(s.Binding.Folder) || !vault.ValidID(s.Binding.HeaderID) || s.Binding.Account == "" || s.Binding.ClientID == "")) {
		clear(key)
		return nil, vault.ErrInvalid
	}
	engine := s.Engine(nil)
	if _, e = engine.Cached(); e != nil {
		clear(key)
		return nil, e
	}
	if e = engine.ValidatePending(); e != nil {
		clear(key)
		return nil, e
	}
	failed = false
	return s, nil
}
func (s *Session) Close() {
	clear(s.Key)
	if s.unlock != nil {
		s.unlock()
		s.unlock = nil
	}
}
func (s *Session) Save() error {
	if privateDir(s.Home) != nil {
		return ErrStorage
	}
	if e := sealBinding(&s.Bundle, s.Header, s.Key, s.Binding); e != nil {
		return e
	}
	data, e := json.Marshal(s.Bundle)
	if e != nil || len(data) > 48*vault.MaxObject {
		return vault.ErrInvalid
	}
	f, e := os.CreateTemp(s.Home, ".vault-*")
	if e != nil {
		return ErrStorage
	}
	path := f.Name()
	defer os.Remove(path)
	defer f.Close()
	if f.Chmod(0600) != nil {
		return ErrStorage
	}
	if _, e = f.Write(data); e != nil {
		return ErrStorage
	}
	if f.Sync() != nil {
		return ErrStorage
	}
	if f.Close() != nil {
		return ErrStorage
	}
	if e = os.Rename(path, filepath.Join(s.Home, "vault.json")); e != nil {
		return ErrStorage
	}
	return syncDir(s.Home)
}

// Backup contains ciphertext only; pending objects remain pending after restore.
func (s *Session) Backup(path string) error {
	b, e := json.Marshal(s.Bundle)
	if e != nil {
		return vault.ErrInvalid
	}
	return exclusive(path, b)
}
func Restore(home, path string, password []byte) error {
	data, e := readPrivate(path)
	if e != nil {
		return e
	}
	var b Bundle
	if e = decode(data, &b); e != nil {
		return e
	}
	if b.Version != 1 {
		return vault.ErrUnsupported
	}
	h, key, e := vault.Unlock(b.Header, password)
	if e != nil {
		return e
	}
	defer clear(key)
	plain, e := vault.Open(key, h.VaultID, h.VaultID, b.Binding)
	if e != nil {
		return e
	}
	defer clear(plain)
	var binding Binding
	if e = decode(plain, &binding); e != nil {
		return e
	}
	binding.Device = vault.NewID()
	// Validate all ciphertext and graph relationships before writing a restore.
	engine := vault.Engine{VaultID: h.VaultID, Key: key, State: b.State}
	if _, e = engine.Cached(); e != nil {
		return e
	}
	if e = engine.ValidatePending(); e != nil {
		return e
	}
	if e = sealBinding(&b, h, key, binding); e != nil {
		return e
	}
	data, e = json.Marshal(b)
	if e != nil {
		return vault.ErrInvalid
	}
	if !filepath.IsAbs(home) {
		return vault.ErrInvalid
	}
	if e = os.Mkdir(home, 0700); e != nil {
		if errors.Is(e, os.ErrExist) {
			return ErrExists
		}
		return ErrStorage
	}
	if e = exclusive(filepath.Join(home, "vault.json"), data); e != nil {
		os.Remove(home)
		return e
	}
	return syncDir(home)
}
