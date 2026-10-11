// Package lab exercises the prototype exclusively with synthetic data.
package lab

import (
	"bytes"
	"context"
	"crypto/sha256"
	"encoding/json"
	"errors"
	"sort"
	"sync"

	"github.com/miles1150/personal-plugins/tools/keyenv/internal/vault"
)

type Memory struct {
	mu            sync.Mutex
	Objects       map[string][]byte
	Offline       bool
	LoseNextReply bool
}

func (m *Memory) Reserve(context.Context) (string, error) {
	m.mu.Lock()
	defer m.mu.Unlock()
	if m.Offline {
		return "", vault.ErrUnavailable
	}
	return vault.NewID(), nil
}
func (m *Memory) List(context.Context) ([]string, error) {
	m.mu.Lock()
	defer m.mu.Unlock()
	if m.Offline {
		return nil, vault.ErrUnavailable
	}
	ids := []string{}
	for id := range m.Objects {
		ids = append(ids, id)
	}
	sort.Strings(ids)
	return ids, nil
}
func (m *Memory) Get(_ context.Context, id string) ([]byte, error) {
	m.mu.Lock()
	defer m.mu.Unlock()
	if m.Offline {
		return nil, vault.ErrUnavailable
	}
	b, ok := m.Objects[id]
	if !ok {
		return nil, vault.ErrIntegrity
	}
	return bytes.Clone(b), nil
}
func (m *Memory) Put(_ context.Context, id string, b []byte) error {
	m.mu.Lock()
	defer m.mu.Unlock()
	if m.Offline {
		return vault.ErrUnavailable
	}
	if m.Objects == nil {
		m.Objects = map[string][]byte{}
	}
	if old, ok := m.Objects[id]; ok && !bytes.Equal(old, b) {
		return vault.ErrIntegrity
	}
	m.Objects[id] = bytes.Clone(b)
	if m.LoseNextReply {
		m.LoseNextReply = false
		return vault.ErrUnavailable
	}
	return nil
}

// FixtureKey is PUBLIC and intentionally suitable only for synthetic probe data.
func FixtureKey() []byte {
	sum := sha256.Sum256([]byte("keyenv-lab-public-fixture-key-not-for-real-secrets"))
	return sum[:]
}

const FixtureVault = "11111111111111111111111111111111"
const FixturePlaintext = "keyenv-lab synthetic token; no real credentials"

func Simulate() (map[string]bool, error) {
	ctx := context.Background()
	key := FixtureKey()
	remote := &Memory{}
	saved := map[string]vault.State{}
	makeEngine := func(device string) *vault.Engine {
		return &vault.Engine{VaultID: FixtureVault, Device: device, Key: key, Store: remote, Save: func(s vault.State) error {
			b, _ := json.Marshal(s)
			var copy vault.State
			_ = json.Unmarshal(b, &copy)
			saved[device] = copy
			return nil
		}}
	}
	a, b := makeEngine("device-alpha"), makeEngine("device-bravo")
	entry := vault.Entry{ID: vault.NewID(), Name: "openai", Fields: map[string]vault.Field{"OPENAI_API_KEY": {Value: FixturePlaintext, Secret: true}}}
	if _, err := a.Prepare(ctx, entry); err != nil {
		return nil, err
	}
	if err := a.Flush(ctx); err != nil {
		return nil, err
	}
	if _, err := a.Prepare(ctx, entry); err != nil {
		return nil, err
	}
	other := entry
	other.Disabled = true
	if _, err := b.Prepare(ctx, other); err != nil {
		return nil, err
	}
	if err := a.Flush(ctx); err != nil {
		return nil, err
	}
	if err := b.Flush(ctx); err != nil {
		return nil, err
	}
	revs, _, err := a.Snapshot(ctx)
	if err != nil {
		return nil, err
	}
	_, err = vault.Select(revs, "openai")
	report := map[string]bool{"same_entry_conflict_retained": errors.Is(err, vault.ErrConflict) && len(revs) == 3}
	remote.Offline = true
	_, stale, err := a.Snapshot(ctx)
	report["offline_cache_read"] = stale && err == nil
	_, err = a.Prepare(ctx, entry)
	report["offline_write_rejected"] = errors.Is(err, vault.ErrUnavailable)
	remote.Offline = false
	c := makeEngine("device-charlie")
	independent := vault.Entry{ID: vault.NewID(), Name: "cloudflare", Fields: map[string]vault.Field{"CLOUDFLARE_API_TOKEN": {Value: FixturePlaintext, Secret: true}}}
	if _, err = c.Prepare(ctx, independent); err != nil {
		return nil, err
	}
	remote.LoseNextReply = true
	err = c.Flush(ctx)
	report["ambiguous_upload_retained"] = errors.Is(err, vault.ErrUnavailable) && len(saved[c.Device].Pending) == 1
	restarted := makeEngine(c.Device)
	restarted.State = saved[c.Device]
	if err = restarted.Flush(ctx); err != nil {
		return nil, err
	}
	report["retry_does_not_duplicate"] = len(remote.Objects) == 4 && len(restarted.State.Pending) == 0
	_, err = vault.Select(revs, "openai")
	report["conflict_blocks_use"] = errors.Is(err, vault.ErrConflict)
	for _, ok := range report {
		if !ok {
			return report, errors.New("SIMULATION_FAILED")
		}
	}
	return report, nil
}
