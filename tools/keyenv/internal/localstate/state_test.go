package localstate

import (
	"bytes"
	"os"
	"path/filepath"
	"testing"

	"github.com/miles1150/personal-plugins/tools/keyenv/internal/lab"
	"github.com/miles1150/personal-plugins/tools/keyenv/internal/vault"
)

func TestPrivateAtomicStateRoundTrip(t *testing.T) {
	dir := t.TempDir()
	if err := os.Chmod(dir, 0700); err != nil {
		t.Fatal(err)
	}
	id := vault.NewID()
	blob, err := vault.Seal(lab.FixtureKey(), lab.FixtureVault, id, []byte("test-secret-value"))
	if err != nil {
		t.Fatal(err)
	}
	path := filepath.Join(dir, "state.json")
	s := vault.State{Confirmed: map[string][]byte{}, Pending: map[string][]byte{id: blob}}
	if err := Save(path, s); err != nil {
		t.Fatal(err)
	}
	info, _ := os.Stat(path)
	if info.Mode().Perm() != 0600 {
		t.Fatal("insecure file mode")
	}
	data, _ := os.ReadFile(path)
	if bytes.Contains(data, []byte("test-secret-value")) {
		t.Fatal("plaintext state")
	}
	loaded, err := Load(path)
	if err != nil || !bytes.Equal(loaded.Pending[id], blob) {
		t.Fatal(err)
	}
	s.Confirmed = s.Pending
	s.Pending = nil
	if err := Save(path, s); err != nil {
		t.Fatal(err)
	}
	loaded, err = Load(path)
	if err != nil || len(loaded.Confirmed) != 1 || len(loaded.Pending) != 0 {
		t.Fatal("replacement failed")
	}
	if err := os.Chmod(path, 0644); err != nil {
		t.Fatal(err)
	}
	if _, err := Load(path); err == nil {
		t.Fatal("public file accepted")
	}
}

func TestUnsafeDirectoryDoesNotCreateState(t *testing.T) {
	dir := t.TempDir()
	if err := os.Chmod(dir, 0755); err != nil {
		t.Fatal(err)
	}
	path := filepath.Join(dir, "state.json")
	if err := Save(path, vault.State{}); err == nil {
		t.Fatal("unsafe directory accepted")
	}
	if _, err := os.Stat(path); !os.IsNotExist(err) {
		t.Fatal("file created")
	}
}
