//go:build !darwin && !linux

package main

import "github.com/miles1150/personal-plugins/tools/keyenv/internal/vault"

type childExit struct{ Code int }

func (e *childExit) Error() string    { return "CHILD_EXIT" }
func launch(args, env []string) error { return vault.ErrUnsupported }
