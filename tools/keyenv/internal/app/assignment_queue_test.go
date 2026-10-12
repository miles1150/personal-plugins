package app

import (
	"bytes"
	"context"
	"errors"
	"os"
	"path/filepath"
	"reflect"
	"testing"

	"github.com/miles1150/personal-plugins/tools/keyenv/internal/lab"
	"github.com/miles1150/personal-plugins/tools/keyenv/internal/vault"
)

type reserveFailure struct{ vault.Store }

func (s reserveFailure) Reserve(context.Context) (string, error) { return "", vault.ErrUnavailable }

func queueValues() map[string]vault.Field {
	return map[string]vault.Field{"SYNTHETIC_FIRST": {Value: "SYNTHETIC-secret-first=$()'", Secret: true}, "SYNTHETIC_SECOND": {Value: "SYNTHETIC-secret-second", Secret: true}}
}
func stageTestAssignments(t *testing.T, s *Session) []AssignmentPlan {
	t.Helper()
	plans, err := PlanAssignments(nil, queueValues())
	if err != nil {
		t.Fatal(err)
	}
	if err = s.StageAssignments(plans); err != nil {
		t.Fatal(err)
	}
	return plans
}

func TestAssignmentDraftSurvivesFailedReserveRestartAndBackup(t *testing.T) {
	s := create(t)
	h := s.Home
	plans := stageTestAssignments(t, s)
	store := &lab.Memory{}
	if err := s.ResumeAssignments(t.Context(), reserveFailure{store}); !errors.Is(err, ErrAssignmentsPending) {
		t.Fatal("reserve failure lost draft state", err)
	}
	if len(s.Binding.Assignments) != 2 || len(s.Bundle.State.Pending) != 0 {
		t.Fatal("unexpected staged state")
	}
	if fields, err := GlobalFields(mustCached(t, s)); err != nil || len(fields) != 0 {
		t.Fatal("draft used as confirmed environment")
	}
	data, _ := os.ReadFile(filepath.Join(h, "vault.json"))
	for k, v := range queueValues() {
		if bytes.Contains(data, []byte(k)) || bytes.Contains(data, []byte(v.Value)) {
			t.Fatal("draft plaintext persisted")
		}
	}
	info, _ := os.Stat(filepath.Join(h, "vault.json"))
	if info.Mode().Perm() != 0600 {
		t.Fatal("draft public")
	}
	backup, restored := filepath.Join(filepath.Dir(h), "draft-backup.json"), filepath.Join(filepath.Dir(h), "restored")
	report, err := s.BackupAndVerifyRestore(backup, restored, testPassword)
	if err != nil || report["pending_assignments_preserved"] != 2 {
		t.Fatal("backup lost draft", err)
	}
	s.Close()
	next, err := Open(h, testPassword)
	if err != nil {
		t.Fatal(err)
	}
	defer next.Close()
	if !reflect.DeepEqual(next.Binding.Assignments, plans) {
		t.Fatal("restart lost batch")
	}
	if err = next.ResumeAssignments(t.Context(), store); err != nil {
		t.Fatal("resume failed", err)
	}
	if len(next.Binding.Assignments) != 0 || len(store.Objects) != 2 {
		t.Fatal("batch incomplete or duplicated")
	}
	fields, err := GlobalFields(mustCached(t, next))
	if err != nil || !reflect.DeepEqual(fields, queueValues()) {
		t.Fatal("confirmed values changed")
	}
}
func mustCached(t *testing.T, s *Session) map[string]vault.Revision {
	t.Helper()
	r, err := s.Engine(nil).Cached()
	if err != nil {
		t.Fatal(err)
	}
	return r
}

func TestAssignmentAmbiguousUploadAndCrashCheckpointDoNotDuplicate(t *testing.T) {
	s := create(t)
	plans := stageTestAssignments(t, s)
	store := &lab.Memory{LoseNextReply: true}
	if err := s.ResumeAssignments(t.Context(), store); !errors.Is(err, ErrAssignmentsPending) {
		t.Fatal("lost reply not preserved", err)
	}
	if len(store.Objects) != 1 || len(s.Bundle.State.Pending) != 1 || len(s.Binding.Assignments) != 2 {
		t.Fatal("lost-reply receipt not recoverable")
	}
	s.Close()
	next, err := Open(s.Home, testPassword)
	if err != nil {
		t.Fatal(err)
	}
	defer next.Close()
	if err = next.ResumeAssignments(t.Context(), store); err != nil {
		t.Fatal(err)
	}
	if len(store.Objects) != 2 || len(next.Binding.Assignments) != 0 {
		t.Fatal("ambiguous upload duplicated")
	}
	// Simulate a crash between confirming revisions and checkpointing the draft.
	next.Binding.Assignments = plans
	if err = next.Save(); err != nil {
		t.Fatal(err)
	}
	if err = next.ResumeAssignments(t.Context(), store); err != nil {
		t.Fatal(err)
	}
	if len(store.Objects) != 2 || len(next.Binding.Assignments) != 0 {
		t.Fatal("committed batch replay duplicated versions")
	}
}

func TestAssignmentDraftRefusesChangedParentsAndDiscardPreservesUploads(t *testing.T) {
	s := create(t)
	store := &lab.Memory{}
	original := envRevision("existing", map[string]vault.Field{"SYNTHETIC_FIRST": {Value: "base", Secret: true}}).Entry
	if _, err := s.Engine(store).PrepareBased(t.Context(), original, nil, false); err != nil {
		t.Fatal(err)
	}
	if err := s.Flush(t.Context(), store); err != nil {
		t.Fatal(err)
	}
	plans, err := PlanAssignments(mustCached(t, s), map[string]vault.Field{"SYNTHETIC_FIRST": {Value: "queued", Secret: true}})
	if err != nil {
		t.Fatal(err)
	}
	if err = s.StageAssignments(plans); err != nil {
		t.Fatal(err)
	}
	other := original
	other.Fields = map[string]vault.Field{"SYNTHETIC_FIRST": {Value: "other-device", Secret: true}}
	if _, err = s.Engine(store).PrepareBased(t.Context(), other, plans[0].Parents, false); err != nil {
		t.Fatal(err)
	}
	if err = s.Flush(t.Context(), store); err != nil {
		t.Fatal(err)
	}
	if err = s.ResumeAssignments(t.Context(), store); !errors.Is(err, vault.ErrConflict) {
		t.Fatal("changed parent silently overwritten", err)
	}
	before := len(store.Objects)
	if err = s.StageAssignments(plans); !errors.Is(err, ErrAssignmentsPending) {
		t.Fatal("draft overwritten")
	}
	if err = s.DiscardAssignments(); err != nil {
		t.Fatal(err)
	}
	if len(store.Objects) != before || len(s.Binding.Assignments) != 0 {
		t.Fatal("discard changed confirmed state")
	}
	current, _ := GlobalFields(mustCached(t, s))
	if current["SYNTHETIC_FIRST"].Value != "other-device" {
		t.Fatal("remote state overwritten")
	}
}
