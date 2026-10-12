// Package terminal only accepts sensitive input from the user's terminal.
package terminal

import (
	"bytes"
	"errors"
	"fmt"
	"os"
	"os/signal"
	"syscall"

	"golang.org/x/term"
)

var ErrInteraction = errors.New("INTERACTION_REQUIRED")
var ErrMismatch = errors.New("PASSWORD_MISMATCH")
var ErrCancelled = errors.New("CANCELLED")

func Read(prompt string, hidden bool) ([]byte, error) {
	if !term.IsTerminal(int(os.Stdin.Fd())) {
		return nil, ErrInteraction
	}
	fmt.Fprint(os.Stderr, prompt)
	if hidden {
		fd := int(os.Stdin.Fd())
		state, err := term.GetState(fd)
		if err != nil {
			return nil, ErrInteraction
		}
		signals := make(chan os.Signal, 1)
		done := make(chan struct{})
		signal.Notify(signals, syscall.SIGINT, syscall.SIGTERM, syscall.SIGHUP, syscall.SIGQUIT)
		go func() {
			select {
			case sig := <-signals:
				_ = term.Restore(fd, state)
				os.Exit(128 + int(sig.(syscall.Signal)))
			case <-done:
			}
		}()
		b, err := term.ReadPassword(fd)
		signal.Stop(signals)
		close(done)
		_ = term.Restore(fd, state)
		fmt.Fprintln(os.Stderr)
		if err != nil || len(b) > 65536 {
			clear(b)
			return nil, ErrInteraction
		}
		return b, nil
	}
	var b []byte
	var one [1]byte
	for len(b) <= 65536 {
		if _, err := os.Stdin.Read(one[:]); err != nil {
			clear(b)
			return nil, ErrInteraction
		}
		if one[0] == '\n' {
			return bytes.TrimSuffix(b, []byte{'\r'}), nil
		}
		b = append(b, one[0])
	}
	clear(b)
	return nil, ErrInteraction
}

func Password(confirm bool) ([]byte, error) {
	p, err := Read("Master password (hidden): ", true)
	if err != nil {
		return nil, err
	}
	if len(p) == 0 || len(p) > 4096 {
		clear(p)
		return nil, ErrInteraction
	}
	if confirm {
		q, e := Read("Confirm master password (hidden): ", true)
		same := bytes.Equal(p, q)
		clear(q)
		if e != nil {
			clear(p)
			return nil, e
		}
		if !same {
			clear(p)
			return nil, ErrMismatch
		}
	}
	return p, nil
}

func Confirm(action string) error {
	b, err := Read(action+" Type yes to confirm: ", false)
	defer clear(b)
	if err != nil {
		return err
	}
	if string(b) != "yes" {
		return ErrCancelled
	}
	return nil
}
