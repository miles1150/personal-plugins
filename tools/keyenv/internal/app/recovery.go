package app

import (
	"bytes"
	"errors"
	"os"
	"path/filepath"
	"reflect"

	"github.com/miles1150/personal-plugins/tools/keyenv/internal/vault"
)

// BackupAndVerifyRestore uses the current process's password for a ciphertext
// backup and an independent on-disk restore. It never connects to Drive or logs
// decrypted values. Existing destinations are refused, including empty folders.
func (s *Session) BackupAndVerifyRestore(output, restoredHome string, password []byte) (map[string]any, error) {
	return s.verifyRecovery(output, restoredHome, func() error { return Restore(restoredHome, output, password) }, func() (*Session, error) { return Open(restoredHome, password) })
}

func (s *Session) BackupAndVerifyRestoreCached(output, restoredHome string) (map[string]any, error) {
	return s.verifyRecovery(output, restoredHome, func() error { return s.RestoreUsingSession(restoredHome, output) }, func() (*Session, error) { return OpenCached(restoredHome) })
}

// RestoreUsingSession restores only a backup with the exact header of this
// unlocked vault. A foreign backup cannot borrow this vault's cached key.
func (s *Session) RestoreUsingSession(home, path string) error {
	data, err := readPrivate(path)
	if err != nil {
		return err
	}
	var bundle Bundle
	if err = decode(data, &bundle); err != nil {
		return err
	}
	if bundle.Version != 1 || !bytes.Equal(bundle.Header, s.Bundle.Header) {
		return vault.ErrIntegrity
	}
	if err = restoreBundle(home, bundle, s.Header, s.Key); err != nil {
		return err
	}
	restored, err := openWith(home, func(header []byte) (vault.Header, []byte, error) {
		if !bytes.Equal(header, s.Bundle.Header) {
			return vault.Header{}, nil, vault.ErrIntegrity
		}
		return s.Header, bytes.Clone(s.Key), nil
	}, false)
	if err != nil {
		return err
	}
	defer restored.Close()
	return restored.SaveUnlockCache()
}

func (s *Session) verifyRecovery(output, restoredHome string, restore func() error, reopen func() (*Session, error)) (map[string]any, error) {
	if !filepath.IsAbs(output) || !filepath.IsAbs(restoredHome) {
		return nil, vault.ErrInvalid
	}
	if _, err := os.Lstat(restoredHome); err == nil {
		return nil, ErrExists
	} else if !errors.Is(err, os.ErrNotExist) {
		return nil, ErrStorage
	}
	original, err := s.Engine(nil).Cached()
	if err != nil {
		return nil, err
	}
	if err = s.Backup(output); err != nil {
		return nil, err
	}
	if err = restore(); err != nil {
		return nil, err
	}
	restored, err := reopen()
	if err != nil {
		return nil, err
	}
	defer restored.Close()
	recovered, err := restored.Engine(nil).Cached()
	if err != nil {
		return nil, err
	}
	beforeBinding, afterBinding := s.Binding, restored.Binding
	deviceChanged := beforeBinding.Device != afterBinding.Device && afterBinding.Device != ""
	beforeBinding.Device, afterBinding.Device = "", ""
	if !deviceChanged || !bytes.Equal(s.Bundle.Header, restored.Bundle.Header) ||
		!reflect.DeepEqual(s.Bundle.State, restored.Bundle.State) ||
		!reflect.DeepEqual(beforeBinding, afterBinding) || !reflect.DeepEqual(original, recovered) {
		return nil, vault.ErrIntegrity
	}
	return map[string]any{
		"result": "backup_and_restore_verified", "recovery_verified": true,
		"ciphertext_only": true, "offline_verification": true,
		"confirmed_objects": len(recovered), "pending_uploads_preserved": len(restored.Bundle.State.Pending),
		"restored_device_changed":   deviceChanged,
		"unlock_material_persisted": s.UnlockCached || restored.UnlockCached, "oauth_token_persisted": false,
	}, nil
}
