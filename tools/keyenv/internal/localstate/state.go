package localstate

import (
	"encoding/json"
	"errors"
	"io"
	"os"
	"path/filepath"

	"github.com/miles1150/personal-plugins/tools/keyenv/internal/vault"
)

var ErrStorage = errors.New("LOCAL_STORAGE_FAILED")

// Save atomically replaces a private ciphertext-only state file. The caller holds a lock.
func Save(path string, state vault.State) error {
	data, err := json.Marshal(state)
	if err != nil || len(data) > 48*vault.MaxObject {
		return ErrStorage
	}
	dir := filepath.Dir(path)
	info, err := os.Lstat(dir)
	if err != nil || !info.IsDir() || info.Mode().Perm()&0077 != 0 {
		return ErrStorage
	}
	f, err := os.CreateTemp(dir, ".keyenv-state-*")
	if err != nil {
		return ErrStorage
	}
	tmp := f.Name()
	defer os.Remove(tmp)
	defer f.Close()
	if err = f.Chmod(0600); err != nil {
		return ErrStorage
	}
	if _, err = f.Write(data); err != nil {
		return ErrStorage
	}
	if err = f.Sync(); err != nil {
		return ErrStorage
	}
	if err = f.Close(); err != nil {
		return ErrStorage
	}
	if err = os.Rename(tmp, path); err != nil {
		return ErrStorage
	}
	d, err := os.Open(dir)
	if err != nil {
		return ErrStorage
	}
	defer d.Close()
	if err = d.Sync(); err != nil {
		return ErrStorage
	}
	return nil
}

func Load(path string) (vault.State, error) {
	var state vault.State
	info, err := os.Lstat(path)
	if err != nil || !info.Mode().IsRegular() || info.Mode().Perm()&0077 != 0 {
		return state, ErrStorage
	}
	f, err := os.Open(path)
	if err != nil {
		return state, ErrStorage
	}
	defer f.Close()
	b, err := io.ReadAll(io.LimitReader(f, 48*vault.MaxObject+1))
	if err != nil || len(b) > 48*vault.MaxObject || json.Unmarshal(b, &state) != nil {
		return state, ErrStorage
	}
	return state, nil
}
