package main

import (
	"bytes"
	"errors"
	"os"
	"os/exec"
	"path/filepath"
	"strings"
	"testing"

	"github.com/miles1150/personal-plugins/tools/keyenv/internal/app"
	"github.com/miles1150/personal-plugins/tools/keyenv/internal/lab"
	"github.com/miles1150/personal-plugins/tools/keyenv/internal/terminal"
	"github.com/miles1150/personal-plugins/tools/keyenv/internal/vault"
)

func TestNewEntryFieldCountRepromptsWithoutEchoingInvalidInput(t *testing.T) {
	inputs := []string{"do-not-echo-this-input", "0", "65", "1"}
	calls := 0
	var feedback bytes.Buffer
	n, err := readFieldCount(false, func(prompt string) (string, error) {
		if !strings.Contains(prompt, "1..64") || calls >= len(inputs) {
			t.Fatal("incorrect new-entry count prompt")
		}
		value := inputs[calls]
		calls++
		return value, nil
	}, &feedback)
	if err != nil || n != 1 || calls != len(inputs) || strings.Count(feedback.String(), "请输入 1 到 64 之间的整数。") != 3 || strings.Contains(feedback.String(), inputs[0]) {
		t.Fatal("invalid count was not safely corrected", n, calls, err)
	}
}

func TestExistingEntryBlankFieldCountKeepsFieldsAndCancellationStops(t *testing.T) {
	var feedback bytes.Buffer
	n, err := readFieldCount(true, func(prompt string) (string, error) {
		if !strings.Contains(prompt, "回车保留现有字段") {
			t.Fatal("missing keep-existing guidance")
		}
		return "", nil
	}, &feedback)
	if err != nil || n != 0 || feedback.Len() != 0 {
		t.Fatal("blank update count should keep fields", n, err)
	}
	_, err = readFieldCount(false, func(string) (string, error) {
		return "", terminal.ErrCancelled
	}, &feedback)
	if !errors.Is(err, terminal.ErrCancelled) {
		t.Fatal("cancelled interaction continued", err)
	}
}

func TestNewEntryBlankCountUsesOneField(t *testing.T) {
	n, err := readFieldCount(false, func(string) (string, error) { return "", nil }, &bytes.Buffer{})
	if err != nil || n != 1 {
		t.Fatal("new entry default count", n, err)
	}
}

func TestNonInteractiveNeverInitializes(t *testing.T) {
	h := filepath.Join(t.TempDir(), "vault")
	for _, args := range [][]string{{"--home", h, "init"}, {"--home", h, "unlock"}, {"--home", h, "restore", "--input", "/not-real-backup"}, {"--home", h, "restore", "--input", "/not-real-backup", "--yes"}} {
		if e := run(args); !errors.Is(e, terminal.ErrInteraction) {
			t.Fatal("noninteractive input accepted", e)
		}
		if _, e := os.Lstat(h); !errors.Is(e, os.ErrNotExist) {
			t.Fatal("created a vault without interaction")
		}
	}
	for _, args := range [][]string{{"--home", h, "add", "synthetic"}, {"--home", h, "backup", "--output", "/not-real-backup", "--verify-restore-home", "/not-real-restore", "--yes"}} {
		if err := run(args); !errors.Is(err, app.ErrUnlockRequired) {
			t.Fatal("maintenance should request setup/unlock, not prompt for a password", err)
		}
		if _, err := os.Lstat(h); !errors.Is(err, os.ErrNotExist) {
			t.Fatal("maintenance created a vault without setup")
		}
	}
	if e := run([]string{"init", "--password", "never-accept-secrets-in-argv"}); !errors.Is(e, vault.ErrInvalid) {
		t.Fatal("password flag accepted", e)
	}
	if e := run([]string{"--home", h, "export", "--yes"}); !errors.Is(e, vault.ErrInvalid) {
		t.Fatal("plaintext export accepted noninteractive confirmation", e)
	}
}

func TestBootstrapSetupRequiresExistingFolderAndPersonalTerminal(t *testing.T) {
	root := t.TempDir()
	h := filepath.Join(root, "vault")
	bundle := filepath.Join(root, "public-bundle")
	if err := run([]string{"--home", h, "setup", "--oauth-bundle", bundle, "--account", "synthetic@example.invalid"}); !errors.Is(err, vault.ErrInvalid) {
		t.Fatal("bootstrap allowed creating another vault", err)
	}
	if err := run([]string{"--home", h, "setup", "--oauth-bundle", bundle, "--account", "synthetic@example.invalid", "--folder", "synthetic-folder"}); !errors.Is(err, terminal.ErrInteraction) {
		t.Fatal("bootstrap bypassed personal interaction", err)
	}
	if _, err := os.Stat(filepath.Join(root, "oauth", "client.json")); !os.IsNotExist(err) {
		t.Fatal("bootstrap wrote plaintext without user unlock")
	}
	os.Mkdir(h, 0700)
	if err := run([]string{"--home", h, "setup", "--oauth-bundle", bundle, "--account", "synthetic@example.invalid", "--folder", "synthetic-folder"}); !errors.Is(err, app.ErrExists) {
		t.Fatal("existing vault was not refused before auth", err)
	}
}

func TestCachedMaintenanceAndRecoveryWithoutTerminal(t *testing.T) {
	if os.Getenv("KEYENV_CACHED_CLI_TEST") == "1" {
		for i, arg := range os.Args {
			if arg == "--" {
				if run(os.Args[i+1:]) != nil {
					os.Exit(27)
				}
				os.Exit(0)
			}
		}
		os.Exit(28)
	}
	root := t.TempDir()
	if err := os.Chmod(root, 0700); err != nil {
		t.Fatal(err)
	}
	h := filepath.Join(root, "vault")
	p := []byte("SYNTHETIC-test-only-password")
	if err := app.Initialize(h, p); err != nil {
		t.Fatal(err)
	}
	s, err := app.Open(h, p)
	if err != nil {
		t.Fatal(err)
	}
	store := &lab.Memory{}
	entry := vault.Entry{ID: vault.NewID(), Name: "synthetic", Fields: map[string]vault.Field{"SYNTHETIC_TOKEN": {Value: "public-test-data", Secret: true}}}
	if _, err = s.Engine(store).PrepareBased(t.Context(), entry, nil, false); err != nil {
		t.Fatal(err)
	}
	if err = s.Flush(t.Context(), store); err != nil {
		t.Fatal(err)
	}
	if err = s.SaveUnlockCache(); err != nil {
		t.Fatal(err)
	}
	s.Close()
	backup, restored := filepath.Join(root, "backup.json"), filepath.Join(root, "restored")
	for _, args := range [][]string{
		{"--home", h, "status"},
		{"--home", h, "list", "--offline"},
		{"--home", h, "keys", "--offline"},
		{"--home", h, "env", "--offline", "--format", "zsh"},
		{"--home", h, "shell-install", "--profile", filepath.Join(root, ".zprofile")},
		{"--home", h, "backup", "--output", backup, "--verify-restore-home", restored, "--yes"},
		{"--home", restored, "inspect", "--offline", "synthetic"},
		{"--home", filepath.Join(root, "another-restore"), "restore", "--input", backup, "--source-home", h, "--yes"},
	} {
		child := exec.Command(os.Args[0], append([]string{"-test.run=^TestCachedMaintenanceAndRecoveryWithoutTerminal$", "--"}, args...)...)
		child.Env = append(os.Environ(), "KEYENV_CACHED_CLI_TEST=1")
		if err = child.Run(); err != nil {
			t.Fatal("cached command required interaction or failed", args[2], err)
		}
	}
	if err = run([]string{"--home", h, "lock"}); err != nil {
		t.Fatal(err)
	}
	if err = run([]string{"--home", h, "list", "--offline"}); !errors.Is(err, app.ErrUnlockRequired) {
		t.Fatal("locked maintenance unexpectedly read or prompted", err)
	}
}

func TestFlatCommandsRejectSecretsInArgumentsAndUnscopedExport(t *testing.T) {
	h := filepath.Join(t.TempDir(), "absent-vault")
	for _, args := range [][]string{
		{"set", "SYNTHETIC_TOKEN=never-in-argv"}, {"keys", "extra"},
		{"env"}, {"env", "--format", "zsh"}, {"env", "--offline"},
		{"env", "--offline", "--format", "json"}, {"shell-install", "extra"},
	} {
		if err := run(append([]string{"--home", h}, args...)); !errors.Is(err, vault.ErrInvalid) {
			t.Fatal("invalid command reached cache or authentication", err)
		}
	}
	if _, err := readAssignments(); !errors.Is(err, terminal.ErrInteraction) {
		t.Fatal("secret input accepted without terminal", err)
	}
}

func TestChildLiteralAndExit(t *testing.T) {
	if os.Getenv("KEYENV_CHILD_TEST") == "1" {
		if os.Getenv("SYNTHETIC_TOKEN") != "$(touch /not-executed);'\"\n`literal`" {
			os.Exit(21)
		}
		if len(os.Args) < 2 || os.Args[len(os.Args)-1] != "$(literal argument)" {
			os.Exit(22)
		}
		os.Exit(17)
	}
	if os.Getenv("KEYENV_LAUNCH_TEST") == "1" {
		_ = launch([]string{os.Args[0], "-test.run=TestChildLiteralAndExit", "--", "$(literal argument)"}, append(os.Environ(), "KEYENV_CHILD_TEST=1", "SYNTHETIC_TOKEN=$(touch /not-executed);'\"\n`literal`"))
		os.Exit(23)
	}
	cmd := exec.Command(os.Args[0], "-test.run=TestChildLiteralAndExit")
	cmd.Env = append(os.Environ(), "KEYENV_LAUNCH_TEST=1")
	err := cmd.Run()
	var exit *exec.ExitError
	if !errors.As(err, &exit) || exit.ExitCode() != 17 {
		t.Fatal("literal args or exit changed", err)
	}
	if e := launch([]string{filepath.Join(t.TempDir(), "missing")}, nil); e == nil {
		t.Fatal("missing executable succeeded")
	}

}

func TestChildSignalExitCode(t *testing.T) {
	if os.Getenv("KEYENV_SIGNAL_TEST") == "1" {
		_ = launch([]string{"/bin/sh", "-c", "kill -TERM $$"}, os.Environ())
		os.Exit(23)
	}
	cmd := exec.Command(os.Args[0], "-test.run=TestChildSignalExitCode")
	cmd.Env = append(os.Environ(), "KEYENV_SIGNAL_TEST=1")
	err := cmd.Run()
	var exit *exec.ExitError
	if !errors.As(err, &exit) || exit.ProcessState.Success() {
		t.Fatal("signal not preserved", err)
	}
}
