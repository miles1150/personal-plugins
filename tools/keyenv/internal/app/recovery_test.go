package app

import (
	"bytes"
	"context"
	"encoding/json"
	"errors"
	"os"
	"path/filepath"
	"testing"

	"github.com/miles1150/personal-plugins/tools/keyenv/internal/lab"
	"github.com/miles1150/personal-plugins/tools/keyenv/internal/vault"
)

func TestBackupRecoveryVerificationPreservesConfirmedPendingAndSource(t *testing.T) {
	s := create(t)
	store := &lab.Memory{}
	entry := testEntry()
	if _, err := s.Engine(store).PrepareBased(context.Background(), entry, nil, false); err != nil {
		t.Fatal(err)
	}
	if err := s.Flush(context.Background(), store); err != nil {
		t.Fatal(err)
	}
	entry.Fields["SYNTHETIC_TOKEN"] = vault.Field{Value: "second-public-test-value", Secret: true}
	confirmed, err := s.Engine(nil).Cached()
	if err != nil {
		t.Fatal(err)
	}
	heads, err := vault.Heads(confirmed)
	if err != nil {
		t.Fatal(err)
	}
	if _, err := s.Engine(store).PrepareBased(context.Background(), entry, heads[entry.Name], false); err != nil {
		t.Fatal(err)
	}
	before, err := os.ReadFile(filepath.Join(s.Home, "vault.json"))
	if err != nil {
		t.Fatal(err)
	}
	backup := filepath.Join(filepath.Dir(s.Home), "verified-backup.json")
	restored := filepath.Join(filepath.Dir(s.Home), "verified-restore")
	report, err := s.BackupAndVerifyRestore(backup, restored, testPassword)
	if err != nil || report["recovery_verified"] != true || report["confirmed_objects"] != 1 || report["pending_uploads_preserved"] != 1 || report["restored_device_changed"] != true {
		t.Fatal("recovery verification failed", err)
	}
	after, _ := os.ReadFile(filepath.Join(s.Home, "vault.json"))
	if !bytes.Equal(before, after) {
		t.Fatal("source vault changed")
	}
	encoded, _ := json.Marshal(report)
	for _, value := range [][]byte{testPassword, []byte(testEntry().Fields["SYNTHETIC_TOKEN"].Value), []byte(entry.Fields["SYNTHETIC_TOKEN"].Value)} {
		data, _ := os.ReadFile(backup)
		if bytes.Contains(encoded, value) || bytes.Contains(data, value) {
			t.Fatal("verification exposed plaintext")
		}
	}
	if _, err = s.BackupAndVerifyRestore(filepath.Join(filepath.Dir(s.Home), "unused-backup.json"), restored, testPassword); !errors.Is(err, ErrExists) {
		t.Fatal("existing restore overwritten", err)
	}
	if _, err = os.Stat(filepath.Join(filepath.Dir(s.Home), "unused-backup.json")); !errors.Is(err, os.ErrNotExist) {
		t.Fatal("existing target created a backup before rejection")
	}
}

func TestBackupRecoveryWrongPasswordDoesNotReportSuccess(t *testing.T) {
	s := create(t)
	restored := filepath.Join(filepath.Dir(s.Home), "bad-restore")
	backup := filepath.Join(filepath.Dir(s.Home), "backup.json")
	report, err := s.BackupAndVerifyRestore(backup, restored, []byte("wrong"))
	if !errors.Is(err, vault.ErrAuthentication) || report != nil {
		t.Fatal("wrong password reported recovery", err)
	}
	if _, err = os.Stat(restored); !errors.Is(err, os.ErrNotExist) {
		t.Fatal("wrong password created restore")
	}
}
