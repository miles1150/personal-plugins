// Package drive is a narrow Drive v3 immutable-object adapter for laboratory use.
package drive

import (
	"bytes"
	"context"
	"encoding/json"
	"errors"
	"io"
	"mime/multipart"
	"net/http"
	"net/textproto"
	"net/url"
	"strings"

	"github.com/miles1150/personal-plugins/tools/keyenv/internal/vault"
)

type Store struct {
	Client   *http.Client
	BaseURL  string
	FolderID string
}

var ErrRemote = errors.New("DRIVE_REQUEST_FAILED")

func (s *Store) request(ctx context.Context, method, path, contentType string, body io.Reader) ([]byte, int, error) {
	base := s.BaseURL
	if base == "" {
		base = "https://www.googleapis.com"
	}
	req, err := http.NewRequestWithContext(ctx, method, base+path, body)
	if err != nil {
		return nil, 0, ErrRemote
	}
	if contentType != "" {
		req.Header.Set("Content-Type", contentType)
	}
	resp, err := s.Client.Do(req)
	if err != nil {
		if ctx.Err() != nil {
			return nil, 0, vault.ErrUnavailable
		}
		return nil, 0, vault.ErrUnavailable
	}
	defer resp.Body.Close()
	data, err := io.ReadAll(io.LimitReader(resp.Body, vault.MaxObject+1))
	if err != nil {
		return nil, resp.StatusCode, vault.ErrUnavailable
	}
	if len(data) > vault.MaxObject {
		return nil, resp.StatusCode, vault.ErrInvalid
	}
	switch {
	case resp.StatusCode == 401 || resp.StatusCode == 403:
		return nil, resp.StatusCode, vault.ErrAuthRequired
	case resp.StatusCode == 429 || resp.StatusCode >= 500:
		return nil, resp.StatusCode, vault.ErrUnavailable
	case resp.StatusCode == 404:
		return nil, resp.StatusCode, vault.ErrIntegrity
	case resp.StatusCode < 200 || resp.StatusCode >= 300:
		return nil, resp.StatusCode, ErrRemote
	}
	return data, resp.StatusCode, nil
}

func (s *Store) Reserve(ctx context.Context) (string, error) {
	b, _, err := s.request(ctx, "GET", "/drive/v3/files/generateIds?count=1&space=drive&type=files", "", nil)
	if err != nil {
		return "", err
	}
	var r struct {
		IDs []string `json:"ids"`
	}
	if json.Unmarshal(b, &r) != nil || len(r.IDs) != 1 || !vault.ValidID(r.IDs[0]) {
		return "", vault.ErrInvalid
	}
	return r.IDs[0], nil
}

func (s *Store) List(ctx context.Context) ([]string, error) {
	if !vault.ValidID(s.FolderID) {
		return nil, vault.ErrInvalid
	}
	var ids []string
	token := ""
	seenTokens := map[string]bool{}
	seenIDs := map[string]bool{}
	for page := 0; page < 100; page++ {
		q := url.Values{"q": {"'" + s.FolderID + "' in parents and trashed = false and appProperties has { key='keyenvLab' and value='1' }"}, "fields": {"nextPageToken,incompleteSearch,files(id)"}, "pageSize": {"100"}, "spaces": {"drive"}}
		if token != "" {
			q.Set("pageToken", token)
		}
		b, _, err := s.request(ctx, "GET", "/drive/v3/files?"+q.Encode(), "", nil)
		if err != nil {
			return nil, err
		}
		var r struct {
			Next       string `json:"nextPageToken"`
			Incomplete bool   `json:"incompleteSearch"`
			Files      []struct {
				ID string `json:"id"`
			} `json:"files"`
		}
		if json.Unmarshal(b, &r) != nil {
			return nil, vault.ErrInvalid
		}
		if r.Incomplete {
			return nil, ErrRemote
		}
		for _, f := range r.Files {
			if !vault.ValidID(f.ID) || seenIDs[f.ID] {
				return nil, vault.ErrInvalid
			}
			seenIDs[f.ID] = true
			ids = append(ids, f.ID)
			if len(ids) > 2048 {
				return nil, vault.ErrInvalid
			}
		}
		if r.Next == "" {
			return ids, nil
		}
		if seenTokens[r.Next] {
			return nil, vault.ErrInvalid
		}
		seenTokens[r.Next] = true
		token = r.Next
	}
	return nil, vault.ErrInvalid
}

func (s *Store) Get(ctx context.Context, id string) ([]byte, error) {
	if !vault.ValidID(id) || !vault.ValidID(s.FolderID) {
		return nil, vault.ErrInvalid
	}
	meta, _, err := s.request(ctx, "GET", "/drive/v3/files/"+id+"?fields=parents,appProperties,trashed", "", nil)
	if err != nil {
		return nil, err
	}
	var m struct {
		Parents    []string          `json:"parents"`
		Properties map[string]string `json:"appProperties"`
		Trashed    bool              `json:"trashed"`
	}
	if json.Unmarshal(meta, &m) != nil || m.Trashed || m.Properties["keyenvLab"] != "1" || len(m.Parents) != 1 || m.Parents[0] != s.FolderID {
		return nil, vault.ErrIntegrity
	}
	b, _, err := s.request(ctx, "GET", "/drive/v3/files/"+id+"?alt=media", "", nil)
	return b, err
}

func (s *Store) Put(ctx context.Context, id string, data []byte) error {
	if !vault.ValidID(id) || !vault.ValidID(s.FolderID) || len(data) > vault.MaxObject {
		return vault.ErrInvalid
	}
	metadata, _ := json.Marshal(map[string]any{"id": id, "name": id + ".enc", "mimeType": "application/octet-stream", "parents": []string{s.FolderID}, "appProperties": map[string]string{"keyenvLab": "1"}})
	var buf bytes.Buffer
	w := multipart.NewWriter(&buf)
	for _, part := range []struct {
		mime string
		data []byte
	}{{"application/json; charset=UTF-8", metadata}, {"application/octet-stream", data}} {
		h := textproto.MIMEHeader{}
		h.Set("Content-Type", part.mime)
		p, err := w.CreatePart(h)
		if err != nil {
			return ErrRemote
		}
		if _, err = p.Write(part.data); err != nil {
			return ErrRemote
		}
	}
	if err := w.Close(); err != nil {
		return ErrRemote
	}
	_, status, err := s.request(ctx, "POST", "/upload/drive/v3/files?uploadType=multipart&fields=id", "multipart/related; boundary="+w.Boundary(), &buf)
	if err != nil && status != http.StatusConflict {
		return err
	}
	// Both normal success and 409 must refer to the exact bytes we attempted to create.
	actual, err := s.Get(ctx, id)
	if err != nil {
		return err
	}
	if !bytes.Equal(actual, data) {
		return vault.ErrIntegrity
	}
	return nil
}

func (s *Store) CreateLabFolder(ctx context.Context) (string, error) {
	data := `{"name":"Keyenv Lab — test data only","mimeType":"application/vnd.google-apps.folder","appProperties":{"keyenvLabFolder":"1"}}`
	b, _, err := s.request(ctx, "POST", "/drive/v3/files?fields=id", "application/json", strings.NewReader(data))
	if err != nil {
		return "", err
	}
	var m struct {
		ID string `json:"id"`
	}
	if json.Unmarshal(b, &m) != nil || !vault.ValidID(m.ID) {
		return "", vault.ErrInvalid
	}
	return m.ID, nil
}

func (s *Store) CheckLabFolder(ctx context.Context) error {
	if !vault.ValidID(s.FolderID) {
		return vault.ErrInvalid
	}
	b, _, err := s.request(ctx, "GET", "/drive/v3/files/"+s.FolderID+"?fields=mimeType,appProperties,trashed", "", nil)
	if err != nil {
		return err
	}
	var m struct {
		Mime       string            `json:"mimeType"`
		Properties map[string]string `json:"appProperties"`
		Trashed    bool              `json:"trashed"`
	}
	if json.Unmarshal(b, &m) != nil || m.Mime != "application/vnd.google-apps.folder" || m.Properties["keyenvLabFolder"] != "1" || m.Trashed {
		return vault.ErrIntegrity
	}
	return nil
}
