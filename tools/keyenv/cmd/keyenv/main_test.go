package main

import (
	"errors"
	"os"
	"os/exec"
	"path/filepath"
	"testing"

	"github.com/miles1150/personal-plugins/tools/keyenv/internal/terminal"
	"github.com/miles1150/personal-plugins/tools/keyenv/internal/vault"
)

func TestNonInteractiveNeverInitializes(t *testing.T) {
	h := filepath.Join(t.TempDir(), "vault")
	for _, args := range [][]string{{"--home", h, "init"}, {"--home", h, "add", "synthetic"}, {"--home", h, "restore", "--input", "/not-real-backup"}} {
		if e := run(args); !errors.Is(e, terminal.ErrInteraction) {
			t.Fatal("noninteractive input accepted", e)
		}
		if _, e := os.Lstat(h); !errors.Is(e, os.ErrNotExist) {
			t.Fatal("created a vault without interaction")
		}
	}
	if e := run([]string{"init", "--password", "never-accept-secrets-in-argv"}); !errors.Is(e, vault.ErrInvalid) {
		t.Fatal("password flag accepted", e)
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
