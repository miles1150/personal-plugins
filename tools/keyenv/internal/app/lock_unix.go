//go:build darwin || linux

package app

import (
	"golang.org/x/sys/unix"
	"os"
	"path/filepath"
)

func lock(home string) (func(), error) {
	fd, e := unix.Open(filepath.Join(home, ".lock"), unix.O_RDWR|unix.O_CREAT|unix.O_NOFOLLOW|unix.O_CLOEXEC, 0600)
	if e != nil {
		return nil, ErrStorage
	}
	f := os.NewFile(uintptr(fd), "vault-lock")
	i, e := f.Stat()
	if e != nil || !i.Mode().IsRegular() || i.Mode().Perm()&0077 != 0 {
		f.Close()
		return nil, ErrStorage
	}
	if e = unix.Flock(fd, unix.LOCK_EX|unix.LOCK_NB); e != nil {
		f.Close()
		return nil, ErrLocked
	}
	return func() { unix.Flock(fd, unix.LOCK_UN); f.Close() }, nil
}
