package app

import (
	"context"
	"errors"
	"reflect"
	"slices"

	"github.com/miles1150/personal-plugins/tools/keyenv/internal/vault"
)

var ErrAssignmentsPending = errors.New("PENDING_ASSIGNMENTS")

// The entire entered batch is encrypted inside Binding before any subsequent
// network request. It is included in ciphertext backups, never in shell exports.
func (s *Session) StageAssignments(plans []AssignmentPlan) error {
	if len(s.Binding.Assignments) != 0 || len(s.Bundle.State.Pending) != 0 {
		return ErrAssignmentsPending
	}
	if len(plans) == 0 {
		return vault.ErrInvalid
	}
	if err := validateAssignments(plans); err != nil {
		return err
	}
	s.Binding.Assignments = plans
	if err := s.Save(); err != nil {
		s.Binding.Assignments = nil
		return err
	}
	return nil
}

func validateAssignments(plans []AssignmentPlan) error {
	if len(plans) > 64 {
		return vault.ErrInvalid
	}
	names, keys := map[string]bool{}, map[string]bool{}
	for _, p := range plans {
		if p.Entry.Validate() != nil || names[p.Entry.Name] || len(p.Parents) > 1 || len(p.Keys) == 0 || len(p.Keys) > 64 {
			return vault.ErrInvalid
		}
		names[p.Entry.Name] = true
		for _, id := range p.Parents {
			if !vault.ValidID(id) {
				return vault.ErrInvalid
			}
		}
		for _, k := range p.Keys {
			if _, ok := p.Entry.Fields[k]; !ok || keys[k] || protectedGlobalKey(k) {
				return vault.ErrInvalid
			}
			keys[k] = true
		}
	}
	if len(keys) > 64 {
		return vault.ErrInvalid
	}
	return nil
}

func (s *Session) AssignmentKeys() []string {
	keys := []string{}
	for _, p := range s.Binding.Assignments {
		keys = append(keys, p.Keys...)
	}
	slices.Sort(keys)
	return keys
}

func pendingAssignmentError(err error, hasDraft bool) error {
	if hasDraft && (errors.Is(err, vault.ErrUnavailable) || errors.Is(err, vault.ErrPending)) {
		return ErrAssignmentsPending
	}
	return err
}

// ResumeAssignments reconciles ambiguous uploads first. Exact revision content
// and expected parents identify a batch step that committed before a crash,
// avoiding a second version. Changed parents fail rather than silently overwrite.
func (s *Session) ResumeAssignments(ctx context.Context, store vault.Store) error {
	return s.ResumeAssignmentsWithProgress(ctx, store, nil)
}

func (s *Session) ResumeAssignmentsWithProgress(ctx context.Context, store vault.Store, progress func(int, int)) error {
	if err := validateAssignments(s.Binding.Assignments); err != nil {
		return err
	}
	total := len(s.Binding.Assignments)
	if progress != nil && total > 0 {
		progress(0, total)
	}
	if err := s.Flush(ctx, store); err != nil {
		return pendingAssignmentError(err, len(s.Binding.Assignments) > 0)
	}
	for len(s.Binding.Assignments) > 0 {
		plan := s.Binding.Assignments[0]
		revs, err := s.Engine(nil).Cached()
		if err != nil {
			return err
		}
		matches := 0
		for _, r := range revs {
			if reflect.DeepEqual(r.Entry, plan.Entry) && slices.Equal(r.Parents, plan.Parents) {
				matches++
			}
		}
		if matches > 1 {
			return vault.ErrConflict
		}
		if matches == 0 {
			if _, err = s.Engine(store).PrepareBased(ctx, plan.Entry, plan.Parents, false); err != nil {
				return pendingAssignmentError(err, true)
			}
			if err = s.Flush(ctx, store); err != nil {
				return pendingAssignmentError(err, true)
			}
		}
		previous := s.Binding.Assignments
		s.Binding.Assignments = previous[1:]
		if len(s.Binding.Assignments) == 0 {
			s.Binding.Assignments = nil
		}
		if err = s.Save(); err != nil {
			s.Binding.Assignments = previous
			return err
		}
		if progress != nil {
			progress(total-len(s.Binding.Assignments), total)
		}
	}
	return nil
}

// Discard only uncompleted intentions; confirmed revisions and queued uploads
// are retained, including a partially successful batch.
func (s *Session) DiscardAssignments() error {
	previous := s.Binding.Assignments
	if len(previous) == 0 {
		return nil
	}
	s.Binding.Assignments = nil
	if err := s.Save(); err != nil {
		s.Binding.Assignments = previous
		return err
	}
	return nil
}
