package app

import (
	"bytes"
	"errors"
	"github.com/miles1150/personal-plugins/tools/keyenv/internal/vault"
	"os"
	"path/filepath"
	"testing"
)

func TestProfileInstallPreservesPrivateBackupAndIsIdempotent(t *testing.T) {
	s := create(t)
	profile := filepath.Join(t.TempDir(), ".zprofile")
	original := []byte("# existing profile\nexport EXISTING_SYNTHETIC=value\n")
	if err := os.WriteFile(profile, original, 0644); err != nil {
		t.Fatal(err)
	}
	binary := "/synthetic/keyenv'cli"
	backup, err := s.InstallProfile(profile, binary)
	if err != nil {
		t.Fatal(err)
	}
	saved, err := os.ReadFile(backup)
	if err != nil || !bytes.Equal(saved, original) {
		t.Fatal("backup mismatch")
	}
	info, _ := os.Stat(backup)
	if info.Mode().Perm() != 0600 {
		t.Fatal("backup public")
	}
	updated, _ := os.ReadFile(profile)
	if !bytes.HasPrefix(updated, original) || bytes.Count(updated, []byte(profileStart)) != 1 {
		t.Fatal("profile overwritten")
	}
	info, _ = os.Stat(profile)
	if info.Mode().Perm() != 0644 {
		t.Fatal("mode changed")
	}
	backup, err = s.InstallProfile(profile, binary)
	if err != nil || backup != "" {
		t.Fatal("nonidempotent installation", err)
	}
	again, _ := os.ReadFile(profile)
	if !bytes.Equal(updated, again) {
		t.Fatal("idempotent content changed")
	}
	suffix := []byte("# appended user customization\n")
	os.WriteFile(profile, append(again, suffix...), 0644)
	_, err = s.InstallProfile(profile, "/synthetic/new-cli")
	if err != nil {
		t.Fatal(err)
	}
	final, _ := os.ReadFile(profile)
	if !bytes.HasPrefix(final, original) || !bytes.HasSuffix(final, suffix) || bytes.Count(final, []byte(profileStart)) != 1 {
		t.Fatal("replacement lost user text")
	}
}
func TestProfileRefusesSymlinkAndMalformedBlock(t *testing.T) {
	s := create(t)
	root := t.TempDir()
	profile := filepath.Join(root, ".zprofile")
	target := filepath.Join(root, "target")
	os.WriteFile(target, []byte("original"), 0600)
	os.Symlink(target, profile)
	if _, err := s.InstallProfile(profile, "/synthetic/cli"); !errors.Is(err, ErrStorage) {
		t.Fatal("symlink accepted")
	}
	data, _ := os.ReadFile(target)
	if string(data) != "original" {
		t.Fatal("symlink target changed")
	}
	os.Remove(profile)
	for _, bad := range []string{profileStart, profileEnd, profileStart + "\n" + profileStart + "\n" + profileEnd} {
		os.WriteFile(profile, []byte(bad), 0600)
		if _, err := s.InstallProfile(profile, "/synthetic/cli"); !errors.Is(err, vault.ErrInvalid) {
			t.Fatal("malformed block accepted")
		}
		data, _ := os.ReadFile(profile)
		if string(data) != bad {
			t.Fatal("malformed profile modified")
		}
	}
}
