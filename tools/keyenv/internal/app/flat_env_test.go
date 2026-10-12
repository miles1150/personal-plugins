package app

import (
	"bytes"
	"errors"
	"os"
	"os/exec"
	"path/filepath"
	"strings"
	"testing"

	"github.com/miles1150/personal-plugins/tools/keyenv/internal/vault"
)

func TestAssignmentLiteralAndValidation(t *testing.T) {
	value := "  '$()=`literal`=中文  "
	key, field, err := ParseAssignment([]byte(" OPENBB_TOEKN =" + value))
	if err != nil || key != "OPENBB_TOEKN" || field.Value != value || !field.Secret {
		t.Fatal("literal assignment changed")
	}
	for _, line := range []string{"NO_EQUALS", "9INVALID=value", "X\nY=value", "A=\x00", "PATH=x", "options=x", "DYLD_INSERT_LIBRARIES=x", "_keyenv_exports=x"} {
		if _, _, err := ParseAssignment([]byte(line)); !errors.Is(err, vault.ErrInvalid) {
			t.Fatal("unsafe assignment accepted")
		}
	}
	_, f, err := ParseAssignment([]byte("EMPTY="))
	if err != nil || f.Value != "" {
		t.Fatal("empty value rejected")
	}
}
func envRevision(name string, fields map[string]vault.Field) vault.Revision {
	return vault.Revision{ID: vault.NewID(), Device: vault.NewID(), Entry: vault.Entry{ID: vault.NewID(), Name: name, Fields: fields}}
}
func TestFlatPlanPreservesServiceAndIndependentKeys(t *testing.T) {
	old := envRevision("openbb", map[string]vault.Field{"OPENBB_TOEKN": {Value: "old", Secret: true}, "OPENBB_ORIGIN": {Value: "https://synthetic.invalid"}})
	old.Entry.Purpose = "kept"
	revs := map[string]vault.Revision{old.ID: old}
	values := map[string]vault.Field{"OPENBB_TOEKN": {Value: "new", Secret: true}, "OTHER_KEY": {Value: "other", Secret: true}, "other_key": {Value: "lowercase", Secret: true}}
	plans, err := PlanAssignments(revs, values)
	if err != nil || len(plans) != 3 {
		t.Fatal("flat plan failed", err)
	}
	names := map[string]bool{}
	for _, p := range plans {
		if names[p.Entry.Name] {
			t.Fatal("case sensitive keys collided")
		}
		names[p.Entry.Name] = true
		if p.Entry.Name == "openbb" && (p.Entry.ID != old.Entry.ID || len(p.Parents) != 1 || p.Parents[0] != old.ID || p.Entry.Purpose != "kept" || p.Entry.Fields["OPENBB_ORIGIN"] != old.Entry.Fields["OPENBB_ORIGIN"] || p.Entry.Fields["OPENBB_TOEKN"].Value != "new") {
			t.Fatal("service fields changed")
		}
	}
	if old.Entry.Fields["OPENBB_TOEKN"].Value != "old" {
		t.Fatal("input graph mutated")
	}
	// Different new keys get different graph names even on separate devices.
	a, _ := PlanAssignments(nil, map[string]vault.Field{"A": {Value: "a"}})
	b, _ := PlanAssignments(nil, map[string]vault.Field{"B": {Value: "b"}})
	if a[0].Entry.Name == b[0].Entry.Name {
		t.Fatal("unrelated keys conflict")
	}
}
func TestFlatPoolRejectsDuplicateConflictDisabledAndInvalidGraph(t *testing.T) {
	a := envRevision("one", map[string]vault.Field{"TOKEN": {Value: "one"}})
	b := envRevision("two", map[string]vault.Field{"TOKEN": {Value: "two"}})
	revs := map[string]vault.Revision{a.ID: a, b.ID: b}
	if _, err := GlobalFields(revs); !errors.Is(err, vault.ErrConflict) {
		t.Fatal("duplicate selected")
	}
	if _, err := PlanAssignments(revs, map[string]vault.Field{"TOKEN": {Value: "new"}}); !errors.Is(err, vault.ErrConflict) {
		t.Fatal("duplicate updated")
	}
	b.Entry.Name = a.Entry.Name
	revs[b.ID] = b
	if _, err := GlobalFields(revs); !errors.Is(err, vault.ErrConflict) {
		t.Fatal("conflict selected")
	}
	a.Entry.Disabled = true
	revs = map[string]vault.Revision{a.ID: a}
	if fields, err := GlobalFields(revs); err != nil || len(fields) != 0 {
		t.Fatal("disabled injected")
	}
	if _, err := PlanAssignments(revs, map[string]vault.Field{"TOKEN": {Value: "new"}}); !errors.Is(err, vault.ErrDisabled) {
		t.Fatal("disabled silently enabled")
	}
	a.Entry.Fields = map[string]vault.Field{"PATH": {Value: "bad"}}
	a.Entry.Disabled = false
	revs[a.ID] = a
	if _, err := GlobalFields(revs); !errors.Is(err, vault.ErrInvalid) {
		t.Fatal("system field injected")
	}
}
func TestZshLiteralGlobalLoadNoExecutionTraceOrStaleValues(t *testing.T) {
	zsh, err := exec.LookPath("zsh")
	if err != nil {
		t.Skip("zsh not installed")
	}
	root := t.TempDir()
	marker := filepath.Join(root, "must-not-exist")
	value := "SYNTHETIC-global-secret-'\"-$HOME-$(touch " + marker + ")-`touch " + marker + "`\n中文=literal"
	fields := map[string]vault.Field{"SYNTHETIC_TOKEN": {Value: value, Secret: true}, "SYNTHETIC_EMPTY": {Value: ""}}
	script, err := ZshEnvironment(fields)
	if err != nil {
		t.Fatal(err)
	}
	exports := filepath.Join(root, "exports")
	os.WriteFile(exports, script, 0600)
	binary := filepath.Join(root, "fake keyenv'cli")
	// Fixture executable returns only synthetic exports, and records no credentials.
	os.WriteFile(binary, []byte("#!/bin/sh\ncat "+ShellQuote(exports)+"\n"), 0700)
	hook, err := ProfileHook(binary, filepath.Join(root, "vault"))
	if err != nil {
		t.Fatal(err)
	}
	hookPath := filepath.Join(root, "hook.zsh")
	os.WriteFile(hookPath, hook, 0600)
	verifier := filepath.Join(root, "verify.sh")
	os.WriteFile(verifier, []byte("#!/bin/sh\n[ \"$SYNTHETIC_TOKEN\" = \"$EXPECTED_SYNTHETIC\" ] && [ \"${SYNTHETIC_EMPTY+x}\" = x ]\n"), 0700)
	command := exec.Command(zsh, "-f", "-x", "-c", "setopt nounset; source "+ShellQuote(hookPath)+"; unsetopt xtrace; "+ShellQuote(verifier)+" || exit 17; print -r -- 'builtin export SYNTHETIC_OTHER=public' > "+ShellQuote(exports)+"; keyenv_load || exit 18; [[ -z ${SYNTHETIC_TOKEN+x} && $SYNTHETIC_OTHER == public ]] || exit 19; setopt xtrace; keyenv_load || exit 20; [[ -o xtrace ]] || exit 21; unsetopt xtrace; exit 0")
	command.Env = append(os.Environ(), "EXPECTED_SYNTHETIC="+value)
	var output bytes.Buffer
	command.Stdout = &output
	command.Stderr = &output
	if err := command.Run(); err != nil {
		t.Fatal("literal load or refresh failed", err, output.String())
	}
	if strings.Contains(output.String(), "SYNTHETIC-global-secret") || strings.Contains(output.String(), "中文=literal") {
		t.Fatal("secret exposed by xtrace")
	}
	if _, err := os.Stat(marker); !os.IsNotExist(err) {
		t.Fatal("value executed")
	}
}
