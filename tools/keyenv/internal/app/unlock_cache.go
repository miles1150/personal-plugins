package app

import (
	"crypto/sha256"
	"encoding/hex"
	"encoding/json"
	"errors"
	"os"
	"path/filepath"

	"github.com/miles1150/personal-plugins/tools/keyenv/internal/vault"
)

const UnlockCacheFile = "unlock-cache.json"

var ErrUnlockRequired = errors.New("LOCAL_UNLOCK_REQUIRED")

// The local cache deliberately contains an unwrapped data key, not the master
// password. It is device-local, owner-private, and excluded from Bundle/backup.
type unlockCache struct {
	Version    int    `json:"version"`
	VaultID    string `json:"vault_id"`
	HeaderHash string `json:"header_sha256"`
	Key        []byte `json:"key"`
}

func headerHash(header []byte) string {
	digest := sha256.Sum256(header)
	return hex.EncodeToString(digest[:])
}

func OpenCached(home string) (*Session, error) {
	if _, err := os.Lstat(home); errors.Is(err, os.ErrNotExist) {
		return nil, ErrUnlockRequired
	}
	return openWith(home, func(header []byte) (vault.Header, []byte, error) {
		h, err := vault.ParseHeader(header)
		if err != nil {
			return h, nil, err
		}
		path := filepath.Join(home, UnlockCacheFile)
		_, err = os.Lstat(path)
		if errors.Is(err, os.ErrNotExist) {
			return h, nil, ErrUnlockRequired
		}
		if err != nil {
			return h, nil, ErrStorage
		}
		data, err := readPrivateLimit(path, 4096)
		if err != nil {
			return h, nil, err
		}
		defer clear(data)
		var cache unlockCache
		if err = decode(data, &cache); err != nil {
			clear(cache.Key)
			return h, nil, err
		}
		if cache.Version != 1 || cache.VaultID != h.VaultID || cache.HeaderHash != headerHash(header) || len(cache.Key) != 32 {
			clear(cache.Key)
			return h, nil, vault.ErrIntegrity
		}
		return h, cache.Key, nil
	}, true)
}

// SaveUnlockCache is used only after explicit setup/init/unlock or a verified
// restore. The session lock serializes cache changes with vault operations.
func (s *Session) SaveUnlockCache() error {
	if privateDir(s.Home) != nil || len(s.Key) != 32 {
		return ErrStorage
	}
	path := filepath.Join(s.Home, UnlockCacheFile)
	if info, err := os.Lstat(path); err == nil {
		if !info.Mode().IsRegular() || info.Mode().Perm()&0077 != 0 {
			return ErrStorage
		}
	} else if !errors.Is(err, os.ErrNotExist) {
		return ErrStorage
	}
	data, err := json.Marshal(unlockCache{Version: 1, VaultID: s.Header.VaultID, HeaderHash: headerHash(s.Bundle.Header), Key: s.Key})
	if err != nil {
		return vault.ErrInvalid
	}
	defer clear(data)
	f, err := os.CreateTemp(s.Home, ".unlock-cache-*")
	if err != nil {
		return ErrStorage
	}
	tmp := f.Name()
	defer os.Remove(tmp)
	defer f.Close()
	if f.Chmod(0600) != nil {
		return ErrStorage
	}
	if _, err = f.Write(data); err != nil || f.Sync() != nil || f.Close() != nil {
		return ErrStorage
	}
	if os.Rename(tmp, path) != nil || syncDir(s.Home) != nil {
		return ErrStorage
	}
	s.UnlockCached = true
	return nil
}

func ClearUnlockCache(home string) error {
	if privateDir(home) != nil {
		return ErrStorage
	}
	release, err := lock(home)
	if err != nil {
		return err
	}
	defer release()
	path := filepath.Join(home, UnlockCacheFile)
	info, err := os.Lstat(path)
	if errors.Is(err, os.ErrNotExist) {
		return nil
	}
	if err != nil || !info.Mode().IsRegular() {
		return ErrStorage
	}
	if os.Remove(path) != nil {
		return ErrStorage
	}
	return syncDir(home)
}
