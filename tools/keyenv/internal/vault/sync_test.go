package vault_test

import (
	"bytes"
	"context"
	"encoding/json"
	"errors"
	"testing"

	"github.com/miles1150/personal-plugins/tools/keyenv/internal/lab"
	"github.com/miles1150/personal-plugins/tools/keyenv/internal/vault"
)

func engine(store vault.Store) *vault.Engine {
	return &vault.Engine{VaultID: lab.FixtureVault, Device: vault.NewID(), Key: lab.FixtureKey(), Store: store, Save: func(vault.State) error { return nil }}
}
func entry(name string) vault.Entry {
	return vault.Entry{ID: vault.NewID(), Name: name, Fields: map[string]vault.Field{"TOKEN": {Value: "private-test-value", Secret: true}}}
}
func put(t *testing.T, e *vault.Engine, v vault.Entry) string {
	t.Helper()
	id, err := e.Prepare(context.Background(), v)
	if err != nil {
		t.Fatal(err)
	}
	if err = e.Flush(context.Background()); err != nil {
		t.Fatal(err)
	}
	return id
}

func TestTwoDevicesAndLostReply(t *testing.T) {
	r, err := lab.Simulate()
	if err != nil {
		t.Fatal(err)
	}
	for name, ok := range r {
		if !ok {
			t.Errorf("%s failed", name)
		}
	}
}

func TestPrepareBasedRejectsUnseenInteractiveChange(t *testing.T) {
	ctx := context.Background()
	store := &lab.Memory{}
	a, b := engine(store), engine(store)
	x := entry("openai")
	first := put(t, a, x)
	if err := b.Pull(ctx); err != nil {
		t.Fatal(err)
	}
	x.Fields["TOKEN"] = vault.Field{Value: "concurrent-synthetic-value", Secret: true}
	put(t, b, x)
	x.Fields["TOKEN"] = vault.Field{Value: "stale-interactive-value", Secret: true}
	if _, err := a.PrepareBased(ctx, x, []string{first}, false); !errors.Is(err, vault.ErrConflict) {
		t.Fatal("unseen revision overwritten", err)
	}
	if len(a.State.Pending) != 0 || len(store.Objects) != 2 {
		t.Fatal("stale edit was queued/uploaded")
	}
}

func TestExplicitSameNameResolutionRetainsHistory(t *testing.T) {
	ctx := context.Background()
	store := &lab.Memory{}
	a, b := engine(store), engine(store)
	x, y := entry("openai"), entry("openai")
	if _, err := a.PrepareBased(ctx, x, nil, false); err != nil {
		t.Fatal(err)
	}
	if _, err := b.PrepareBased(ctx, y, nil, false); err != nil {
		t.Fatal(err)
	}
	if err := a.Flush(ctx); err != nil {
		t.Fatal(err)
	}
	if err := b.Flush(ctx); err != nil {
		t.Fatal(err)
	}
	revs, _, err := a.Snapshot(ctx)
	if err != nil {
		t.Fatal(err)
	}
	heads, _ := vault.Heads(revs)
	if _, err = a.PrepareBased(ctx, x, heads[x.Name], false); !errors.Is(err, vault.ErrConflict) {
		t.Fatal("implicit merge allowed", err)
	}
	if _, err = a.PrepareBased(ctx, x, heads[x.Name], true); err != nil {
		t.Fatal(err)
	}
	if err = a.Flush(ctx); err != nil {
		t.Fatal(err)
	}
	revs, _, err = a.Snapshot(ctx)
	if err != nil {
		t.Fatal(err)
	}
	chosen, err := vault.Select(revs, x.Name)
	if err != nil || chosen.ID != x.ID || len(revs) != 3 || len(store.Objects) != 3 {
		t.Fatal("resolution lost original history", err)
	}
	// A device which did not see the explicit merge must not overwrite it.
	if _, err = b.PrepareBased(ctx, y, heads[y.Name], false); !errors.Is(err, vault.ErrConflict) {
		t.Fatal("old collision resurrected silently", err)
	}
}

func TestDifferentEntriesMergeAndSameNameConflicts(t *testing.T) {
	for _, sameName := range []bool{false, true} {
		t.Run(map[bool]string{true: "same-name", false: "different-entries"}[sameName], func(t *testing.T) {
			store := &lab.Memory{}
			a, b := engine(store), engine(store)
			x, y := entry("openai"), entry("cloudflare")
			if sameName {
				y.Name = x.Name
			}
			if _, err := a.Prepare(context.Background(), x); err != nil {
				t.Fatal(err)
			}
			if _, err := b.Prepare(context.Background(), y); err != nil {
				t.Fatal(err)
			}
			if err := a.Flush(context.Background()); err != nil {
				t.Fatal(err)
			}
			if err := b.Flush(context.Background()); err != nil {
				t.Fatal(err)
			}
			revs, _, err := a.Snapshot(context.Background())
			if err != nil {
				t.Fatal(err)
			}
			_, err = vault.Select(revs, "openai")
			if sameName && !errors.Is(err, vault.ErrConflict) {
				t.Fatal("same-name collision was silently merged")
			}
			if !sameName && err != nil {
				t.Fatal(err)
			}
			if len(revs) != 2 {
				t.Fatal("lost revision")
			}
		})
	}
}

func TestDisabledAndRedactedSummary(t *testing.T) {
	e := engine(&lab.Memory{})
	x := entry("openai")
	put(t, e, x)
	x.Disabled = true
	put(t, e, x)
	revs, _, err := e.Snapshot(context.Background())
	if err != nil {
		t.Fatal(err)
	}
	if _, err = vault.Select(revs, "openai"); !errors.Is(err, vault.ErrDisabled) {
		t.Fatal(err)
	}
	summaries, err := vault.Summaries(revs)
	if err != nil {
		t.Fatal(err)
	}
	data, _ := json.Marshal(summaries)
	if bytes.Contains(data, []byte("private-test-value")) {
		t.Fatal("summary leaks secret")
	}
	state, _ := json.Marshal(e.State)
	if bytes.Contains(state, []byte("private-test-value")) || bytes.Contains(state, []byte("openai")) {
		t.Fatal("local state leaks plaintext")
	}
}

func TestCorruptionNeverFallsBackToCache(t *testing.T) {
	store := &lab.Memory{}
	e := engine(store)
	id := put(t, e, entry("openai"))
	old, _ := json.Marshal(e.State)
	store.Objects[id][0] ^= 1
	if _, stale, err := e.Snapshot(context.Background()); err == nil || stale {
		t.Fatal("corruption treated as offline")
	}
	after, _ := json.Marshal(e.State)
	if !bytes.Equal(old, after) {
		t.Fatal("corruption replaced valid cache")
	}
}

type brokenStore struct {
	vault.Store
	listErr error
	getErr  error
}

func (s brokenStore) List(ctx context.Context) ([]string, error) {
	if s.listErr != nil {
		return nil, s.listErr
	}
	return s.Store.List(ctx)
}
func (s brokenStore) Get(ctx context.Context, id string) ([]byte, error) {
	if s.getErr != nil {
		return nil, s.getErr
	}
	return s.Store.Get(ctx, id)
}

func TestAuthFallbackPartialPullAndSaveFailure(t *testing.T) {
	ctx := context.Background()
	store := &lab.Memory{}
	a := engine(store)
	put(t, a, entry("openai"))
	a.Store = brokenStore{Store: store, listErr: vault.ErrAuthRequired}
	if _, stale, err := a.Snapshot(ctx); err != nil || !stale {
		t.Fatal("auth cache fallback failed", err)
	}
	if _, err := a.Prepare(ctx, entry("cloudflare")); !errors.Is(err, vault.ErrAuthRequired) {
		t.Fatal("offline modification allowed")
	}
	a.Store = store
	b := engine(store)
	put(t, b, entry("cloudflare"))
	old, _ := json.Marshal(a.State)
	a.Store = brokenStore{Store: store, getErr: vault.ErrUnavailable}
	if _, stale, err := a.Snapshot(ctx); err != nil || !stale {
		t.Fatal(err)
	}
	after, _ := json.Marshal(a.State)
	if !bytes.Equal(old, after) {
		t.Fatal("partial pull changed cache")
	}
	a.Store = store
	a.Save = func(vault.State) error { return errors.New("disk full") }
	if _, err := a.Prepare(ctx, entry("third")); err == nil {
		t.Fatal("write proceeded without durable state")
	}
	if len(store.Objects) != 2 {
		t.Fatal("save failure uploaded data")
	}
}

func TestQueueSurvivesRestartAndIsNeverUsedBeforeConfirmation(t *testing.T) {
	ctx := context.Background()
	store := &lab.Memory{}
	a := engine(store)
	x := entry("openai")
	put(t, a, x)
	x.Fields = map[string]vault.Field{"TOKEN": {Value: "new-private-test-value", Secret: true}}
	if _, err := a.Prepare(ctx, x); err != nil {
		t.Fatal(err)
	}
	encoded, _ := json.Marshal(a.State)
	b := engine(store)
	if err := json.Unmarshal(encoded, &b.State); err != nil {
		t.Fatal(err)
	}
	store.Offline = true
	revs, stale, err := b.Snapshot(ctx)
	if err != nil || !stale {
		t.Fatal(err)
	}
	selected, err := vault.Select(revs, "openai")
	if err != nil {
		t.Fatal(err)
	}
	if selected.Fields["TOKEN"].Value != "private-test-value" {
		t.Fatal("pending data used as confirmed")
	}
	store.Offline = false
	if err := b.Flush(ctx); err != nil {
		t.Fatal(err)
	}
	revs, _, _ = b.Snapshot(ctx)
	selected, err = vault.Select(revs, "openai")
	if err != nil || selected.Fields["TOKEN"].Value != "new-private-test-value" {
		t.Fatal("retry failed")
	}
}

func TestGraphValidationAndResolution(t *testing.T) {
	e := entry("openai")
	root := vault.Revision{ID: "revision-root", Device: "device-one", Entry: e}
	a := vault.Revision{ID: "revision-alpha", Device: "device-one", Parents: []string{root.ID}, Entry: e}
	b := vault.Revision{ID: "revision-bravo", Device: "device-two", Parents: []string{root.ID}, Entry: e}
	revs := map[string]vault.Revision{root.ID: root, a.ID: a, b.ID: b}
	if _, err := vault.Select(revs, "openai"); !errors.Is(err, vault.ErrConflict) {
		t.Fatal(err)
	}
	merged := vault.Revision{ID: "revision-merged", Device: "device-one", Parents: []string{a.ID, b.ID}, Entry: e}
	revs[merged.ID] = merged
	if _, err := vault.Select(revs, "openai"); err != nil {
		t.Fatal("resolution did not cover both heads", err)
	}
	delete(revs, root.ID)
	if _, err := vault.Heads(revs); err == nil {
		t.Fatal("missing parent accepted")
	}
	root.Parents = []string{merged.ID}
	revs[root.ID] = root
	if _, err := vault.Heads(revs); err == nil {
		t.Fatal("cycle accepted")
	}
	root.Parents = nil
	revs[root.ID] = root
	a.Entry = entry("other")
	revs[a.ID] = a
	if _, err := vault.Heads(revs); err == nil {
		t.Fatal("cross-entry parent accepted")
	}
}
