//go:build darwin || linux

package remote

import (
	"os"
	"path/filepath"

	"golang.org/x/sys/unix"
)

// lockHome serializes token refresh: a rotated refresh token is single-use.
func lockHome(home string) (func(), error) {
	if err := os.MkdirAll(home, 0o700); err != nil {
		return nil, err
	}
	fd, err := unix.Open(filepath.Join(home, ".lock"), unix.O_RDWR|unix.O_CREAT|unix.O_NOFOLLOW|unix.O_CLOEXEC, 0o600)
	if err != nil {
		return nil, err
	}
	if err = unix.Flock(fd, unix.LOCK_EX); err != nil {
		unix.Close(fd)
		return nil, err
	}
	return func() { unix.Flock(fd, unix.LOCK_UN); unix.Close(fd) }, nil
}
