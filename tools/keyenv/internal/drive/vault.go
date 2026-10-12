package drive

import (
	"context"
	"encoding/json"
	"strings"

	"github.com/miles1150/personal-plugins/tools/keyenv/internal/vault"
)

func (s *Store) Account(ctx context.Context) (string, error) {
	b, _, e := s.request(ctx, "GET", "/drive/v3/about?fields=user(emailAddress)", "", nil)
	if e != nil {
		return "", e
	}
	var r struct {
		User struct {
			Email string `json:"emailAddress"`
		} `json:"user"`
	}
	if json.Unmarshal(b, &r) != nil || r.User.Email == "" {
		return "", vault.ErrAuthRequired
	}
	return r.User.Email, nil
}

// CreateVaultFolder uses a distinct marker; a laboratory folder is never a vault.
func (s *Store) CreateVaultFolder(ctx context.Context, vaultID string) (string, error) {
	id, e := s.Reserve(ctx)
	if e != nil {
		return "", e
	}
	if e = s.CreateVaultFolderAt(ctx, id, vaultID); e != nil {
		return "", e
	}
	return id, nil
}

// CreateVaultFolderAt resumes an initialization with a saved pre-generated ID.
func (s *Store) CreateVaultFolderAt(ctx context.Context, id, vaultID string) error {
	if !vault.ValidID(vaultID) {
		return vault.ErrInvalid
	}
	if !vault.ValidID(id) {
		return vault.ErrInvalid
	}
	data, _ := json.Marshal(map[string]any{"id": id, "name": "Keyenv", "mimeType": "application/vnd.google-apps.folder", "appProperties": map[string]string{"keyenvVaultFolder": vaultID}})
	_, status, e := s.request(ctx, "POST", "/drive/v3/files?fields=id", "application/json", strings.NewReader(string(data)))
	if e != nil && status != 409 {
		return e
	}
	check := *s
	check.FolderID = id
	actual, e := check.VaultFolder(ctx)
	if e != nil {
		return e
	}
	if actual != vaultID {
		return vault.ErrIntegrity
	}
	return nil
}

func (s *Store) VaultFolder(ctx context.Context) (string, error) {
	if !vault.ValidID(s.FolderID) {
		return "", vault.ErrInvalid
	}
	b, _, e := s.request(ctx, "GET", "/drive/v3/files/"+s.FolderID+"?fields=mimeType,appProperties,trashed", "", nil)
	if e != nil {
		return "", e
	}
	var r struct {
		Mime    string            `json:"mimeType"`
		Props   map[string]string `json:"appProperties"`
		Trashed bool              `json:"trashed"`
	}
	if json.Unmarshal(b, &r) != nil || r.Trashed || r.Mime != "application/vnd.google-apps.folder" || !vault.ValidID(r.Props["keyenvVaultFolder"]) {
		return "", vault.ErrIntegrity
	}
	return r.Props["keyenvVaultFolder"], nil
}

// ReadHeader requires exactly one explicit header; never discovers a vault by name.
func (s *Store) ReadHeader(ctx context.Context, vaultID string) (string, []byte, error) {
	h := *s
	h.Marker = "keyenvHeader"
	h.Value = vaultID
	ids, e := h.List(ctx)
	if e != nil {
		return "", nil, e
	}
	if len(ids) != 1 {
		return "", nil, vault.ErrIntegrity
	}
	b, e := h.Get(ctx, ids[0])
	return ids[0], b, e
}

func (s *Store) WriteHeader(ctx context.Context, id, vaultID string, data []byte) error {
	h := *s
	h.Marker = "keyenvHeader"
	h.Value = vaultID
	return h.Put(ctx, id, data)
}
