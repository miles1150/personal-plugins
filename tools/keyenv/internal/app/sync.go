package app

import (
	"bytes"
	"context"
	"errors"
	"io"
	"net/http"
	"time"

	"github.com/miles1150/personal-plugins/tools/keyenv/internal/drive"
	"github.com/miles1150/personal-plugins/tools/keyenv/internal/vault"
)

// Engine persists ciphertext before upload. Its Session owns the interprocess lock.
func (s *Session) Engine(store vault.Store) *vault.Engine {
	return &vault.Engine{VaultID: s.Header.VaultID, Device: s.Binding.Device, Key: s.Key, Store: store, State: s.Bundle.State, Save: func(state vault.State) error {
		old := s.Bundle.State
		s.Bundle.State = state
		if e := s.Save(); e != nil {
			s.Bundle.State = old
			return e
		}
		return nil
	}}
}

func (s *Session) Remote(ctx context.Context, instructions io.Writer) (*drive.Store, error) {
	if s.Binding.SetupPending {
		return nil, vault.ErrPending
	}
	if s.Binding.Folder == "" {
		return nil, vault.ErrAuthRequired
	}
	id, e := drive.ClientIdentity(s.Binding.ClientPath)
	if e != nil {
		return nil, e
	}
	if id != s.Binding.ClientID {
		return nil, vault.ErrIntegrity
	}
	c, e := s.Authenticate(ctx, instructions, false)
	if e != nil {
		return nil, e
	}
	store := &drive.Store{Client: c, FolderID: s.Binding.Folder, Marker: "keyenvRevision", Value: s.Header.VaultID}
	account, e := store.Account(ctx)
	if e != nil {
		return nil, e
	}
	if account != s.Binding.Account {
		return nil, vault.ErrAuthRequired
	}
	id, e = store.VaultFolder(ctx)
	if e != nil {
		return nil, e
	}
	if id != s.Header.VaultID {
		return nil, vault.ErrIntegrity
	}
	h := *store
	h.Marker = "keyenvHeader"
	b, e := h.Get(ctx, s.Binding.HeaderID)
	if e != nil {
		return nil, e
	}
	if !bytes.Equal(b, s.Bundle.Header) {
		return nil, vault.ErrIntegrity
	}
	return store, nil
}

// Snapshot only falls back for unavailable/auth-required, never damaged local/remote data.
func (s *Session) Snapshot(ctx context.Context, store vault.Store, remoteErr error) (map[string]vault.Revision, bool, error) {
	e := s.Engine(store)
	if remoteErr != nil {
		if !errors.Is(remoteErr, vault.ErrUnavailable) && !errors.Is(remoteErr, vault.ErrAuthRequired) {
			return nil, false, remoteErr
		}
		if len(s.Bundle.State.Confirmed) == 0 {
			return nil, true, remoteErr
		}
		r, err := e.Cached()
		return r, true, err
	}
	r, stale, err := e.Snapshot(ctx)
	if err != nil {
		return nil, stale, err
	}
	if !stale {
		s.Bundle.LastPull = time.Now().UTC()
		if err = s.Save(); err != nil {
			return nil, false, err
		}
	}
	return r, stale, nil
}

func (s *Session) Flush(ctx context.Context, store vault.Store) error {
	e := s.Engine(store)
	if err := e.Flush(ctx); err != nil {
		if len(e.State.Pending) > 0 && (errors.Is(err, vault.ErrUnavailable) || errors.Is(err, vault.ErrAuthRequired)) {
			return vault.ErrPending
		}
		return err
	}
	s.Bundle.LastPull = time.Now().UTC()
	return s.Save()
}

// Authenticate obtains a device-local encrypted OAuth grant without changing
// vault objects or expanding the existing client's permissions.
func (s *Session) Authenticate(ctx context.Context, instructions io.Writer, force bool) (*http.Client, error) {
	id, err := drive.ClientIdentity(s.Binding.ClientPath)
	if err != nil {
		return nil, err
	}
	if id != s.Binding.ClientID || s.Binding.Account == "" {
		return nil, vault.ErrIntegrity
	}
	return drive.AuthorizeCachedWithConsent(ctx, s.Binding.ClientPath, s.Binding.Account, instructions, s.LoginCache(s.Binding.Account, id), force)
}

func (s *Session) RefreshAuthentication(ctx context.Context) error {
	id, err := drive.ClientIdentity(s.Binding.ClientPath)
	if err != nil {
		return err
	}
	if id != s.Binding.ClientID || s.Binding.Account == "" {
		return vault.ErrIntegrity
	}
	client, err := drive.RefreshCached(ctx, s.Binding.ClientPath, s.Binding.Account, s.LoginCache(s.Binding.Account, id))
	if err != nil {
		return err
	}
	actual, err := (&drive.Store{Client: client}).Account(ctx)
	if err != nil {
		return err
	}
	if actual != s.Binding.Account {
		return vault.ErrAuthRequired
	}
	return nil
}
