//go:build darwin || linux

package main

import (
	"errors"
	"os/exec"
	"syscall"
)

type childExit struct{ Code int }

func (e *childExit) Error() string { return "CHILD_EXIT" }

// Replace the unlocked CLI after its vault session has closed. The target keeps
// the PID and foreground terminal group, so signals and exit status are native.
func launch(args, env []string) error {
	path, err := exec.LookPath(args[0])
	if err != nil {
		return errors.New("CHILD_START_FAILED")
	}
	if err = syscall.Exec(path, args, env); err != nil {
		return errors.New("CHILD_START_FAILED")
	}
	return nil
}
