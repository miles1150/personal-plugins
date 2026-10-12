package vault

import (
	"bytes"
	"context"
	"encoding/json"
	"errors"
	"sort"
)

var (
	ErrUnavailable  = errors.New("REMOTE_UNAVAILABLE")
	ErrAuthRequired = errors.New("AUTH_REQUIRED")
	ErrPending      = errors.New("PENDING_UPLOAD")
	ErrIntegrity    = errors.New("INTEGRITY_ERROR")
)

// Store is an immutable object store. Put must be idempotent and reject differing bytes.
type Store interface {
	Reserve(context.Context) (string, error)
	List(context.Context) ([]string, error)
	Get(context.Context, string) ([]byte, error)
	Put(context.Context, string, []byte) error
}

// State contains ciphertext only. It may be serialized to private local storage.
type State struct {
	Confirmed map[string][]byte `json:"confirmed"`
	Pending   map[string][]byte `json:"pending"`
}

// Engine is single-owner: a CLI must hold an interprocess lock for its lifetime.
type Engine struct {
	VaultID string
	Device  string
	Key     []byte
	Store   Store
	State   State
	Save    func(State) error
}

func clone(m map[string][]byte) map[string][]byte {
	out := make(map[string][]byte, len(m))
	for id, b := range m {
		out[id] = bytes.Clone(b)
	}
	return out
}

func (e *Engine) save(s State) error {
	if e.Save == nil {
		return ErrInvalid
	}
	if err := e.Save(s); err != nil {
		return err
	}
	e.State = s
	return nil
}

func (e *Engine) decodeAll(blobs map[string][]byte) (map[string]Revision, error) {
	if len(blobs) > 2048 {
		return nil, ErrInvalid
	}
	revs := make(map[string]Revision, len(blobs))
	var total int
	for id, blob := range blobs {
		total += len(blob)
		if total > 32*MaxObject {
			return nil, ErrInvalid
		}
		plain, err := Open(e.Key, e.VaultID, id, blob)
		if err != nil {
			return nil, err
		}
		var r Revision
		err = decode(plain, &r)
		clear(plain)
		if err != nil || r.ID != id {
			return nil, ErrInvalid
		}
		revs[id] = r
	}
	_, err := Heads(revs)
	return revs, err
}

func (e *Engine) Pull(ctx context.Context) error {
	ids, err := e.Store.List(ctx)
	if err != nil {
		return err
	}
	if len(ids) > 2048 {
		return ErrInvalid
	}
	confirmed := clone(e.State.Confirmed)
	pending := clone(e.State.Pending)
	seen := map[string]bool{}
	for _, id := range ids {
		if !ValidID(id) || seen[id] {
			return ErrInvalid
		}
		seen[id] = true
		b, err := e.Store.Get(ctx, id)
		if err != nil {
			return err
		}
		if old, ok := confirmed[id]; ok && !bytes.Equal(old, b) {
			return ErrIntegrity
		}
		if old, ok := pending[id]; ok && !bytes.Equal(old, b) {
			return ErrIntegrity
		}
		confirmed[id] = b
		delete(pending, id)
	}
	if _, err := e.decodeAll(confirmed); err != nil {
		return err
	}
	if err := e.save(State{Confirmed: confirmed, Pending: pending}); err != nil {
		return err
	}
	return nil
}

// Snapshot permits cache fallback only for connection/authentication errors, not corruption.
func (e *Engine) Snapshot(ctx context.Context) (map[string]Revision, bool, error) {
	err := e.Pull(ctx)
	stale := err != nil
	if err != nil && !errors.Is(err, ErrUnavailable) && !errors.Is(err, ErrAuthRequired) {
		return nil, false, err
	}
	if stale && len(e.State.Confirmed) == 0 {
		return nil, true, err
	}
	revs, err := e.decodeAll(e.State.Confirmed)
	return revs, stale, err
}

// Cached never contacts the store; callers must report that this is an offline read.
func (e *Engine) Cached() (map[string]Revision, error) { return e.decodeAll(e.State.Confirmed) }

func (e *Engine) ValidatePending() error {
	combined := clone(e.State.Confirmed)
	for id, b := range e.State.Pending {
		if old, ok := combined[id]; ok && !bytes.Equal(old, b) {
			return ErrIntegrity
		}
		combined[id] = b
	}
	_, err := e.decodeAll(combined)
	return err
}

// PrepareBased refuses to apply a stale interactive edit to unseen parents.
// A resolution explicitly merges all current same-name heads, retaining every history object.
func (e *Engine) PrepareBased(ctx context.Context, entry Entry, expected []string, resolve bool) (string, error) {
	if err := e.Pull(ctx); err != nil {
		return "", err
	}
	if len(e.State.Pending) != 0 {
		return "", ErrPending
	}
	if err := entry.Validate(); err != nil {
		return "", err
	}
	revs, err := e.Cached()
	if err != nil {
		return "", err
	}
	heads, err := Heads(revs)
	if err != nil {
		return "", err
	}
	parents := heads[entry.Name]
	a, b := append([]string(nil), expected...), append([]string(nil), parents...)
	sort.Strings(a)
	sort.Strings(b)
	if len(a) != len(b) {
		return "", ErrConflict
	}
	for i := range a {
		if a[i] != b[i] {
			return "", ErrConflict
		}
	}
	if !resolve && len(parents) > 1 {
		return "", ErrConflict
	}
	if resolve && len(parents) < 2 {
		return "", ErrInvalid
	}
	chosenIDSeen := len(parents) == 0
	for _, p := range parents {
		if revs[p].Entry.ID == entry.ID {
			chosenIDSeen = true
		}
		if !resolve && revs[p].Entry.ID != entry.ID {
			return "", ErrConflict
		}
	}
	if !chosenIDSeen {
		return "", ErrConflict
	}
	for _, r := range revs {
		if r.Entry.ID == entry.ID && r.Entry.Name != entry.Name {
			return "", ErrInvalid
		}
	}
	id, err := e.Store.Reserve(ctx)
	if err != nil {
		return "", err
	}
	if _, ok := e.State.Confirmed[id]; ok {
		return "", ErrIntegrity
	}
	r := Revision{ID: id, Device: e.Device, Parents: parents, Entry: entry, Resolution: resolve}
	plain, err := json.Marshal(r)
	if err != nil {
		return "", ErrInvalid
	}
	defer clear(plain)
	sealed, err := Seal(e.Key, e.VaultID, id, plain)
	if err != nil {
		return "", err
	}
	pending := clone(e.State.Pending)
	pending[id] = sealed
	if err = e.save(State{Confirmed: clone(e.State.Confirmed), Pending: pending}); err != nil {
		return "", err
	}
	return id, nil
}

// Prepare requires a successful pull in this operation, then saves ciphertext before any upload.
func (e *Engine) Prepare(ctx context.Context, entry Entry) (string, error) {
	if err := e.Pull(ctx); err != nil {
		return "", err
	}
	if len(e.State.Pending) != 0 {
		return "", ErrPending
	}
	if err := entry.Validate(); err != nil {
		return "", err
	}
	revs, err := e.decodeAll(e.State.Confirmed)
	if err != nil {
		return "", err
	}
	heads, _ := Heads(revs)
	parents := heads[entry.Name]
	if len(parents) > 1 {
		return "", ErrConflict
	}
	if len(parents) == 1 && revs[parents[0]].Entry.ID != entry.ID {
		return "", ErrConflict
	}
	// Renaming an existing ID is not part of the prototype.
	for _, r := range revs {
		if r.Entry.ID == entry.ID && r.Entry.Name != entry.Name {
			return "", ErrInvalid
		}
	}
	id, err := e.Store.Reserve(ctx)
	if err != nil {
		return "", err
	}
	if _, ok := e.State.Confirmed[id]; ok {
		return "", ErrIntegrity
	}
	r := Revision{ID: id, Device: e.Device, Parents: parents, Entry: entry}
	plain, err := json.Marshal(r)
	if err != nil {
		return "", ErrInvalid
	}
	defer clear(plain)
	sealed, err := Seal(e.Key, e.VaultID, id, plain)
	if err != nil {
		return "", err
	}
	pending := clone(e.State.Pending)
	pending[id] = sealed
	if err := e.save(State{Confirmed: clone(e.State.Confirmed), Pending: pending}); err != nil {
		return "", err
	}
	return id, nil
}

func (e *Engine) Flush(ctx context.Context) error {
	// First reconcile a previous successful upload whose response was lost.
	if err := e.Pull(ctx); err != nil {
		return err
	}
	ids := make([]string, 0, len(e.State.Pending))
	for id := range e.State.Pending {
		ids = append(ids, id)
	}
	sort.Strings(ids)
	for _, id := range ids {
		blob := e.State.Pending[id]
		// Validate queued objects and parents before sending anything.
		combined := clone(e.State.Confirmed)
		combined[id] = blob
		if _, err := e.decodeAll(combined); err != nil {
			return err
		}
		if err := e.Store.Put(ctx, id, blob); err != nil {
			return err
		}
		actual, err := e.Store.Get(ctx, id)
		if err != nil {
			return err
		}
		if !bytes.Equal(actual, blob) {
			return ErrIntegrity
		}
		confirmed := clone(e.State.Confirmed)
		confirmed[id] = blob
		pending := clone(e.State.Pending)
		delete(pending, id)
		if err := e.save(State{Confirmed: confirmed, Pending: pending}); err != nil {
			return err
		}
	}
	return e.Pull(ctx)
}
