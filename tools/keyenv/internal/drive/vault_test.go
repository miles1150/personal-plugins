package drive

import (
	"context"
	"encoding/json"
	"errors"
	"io"
	"net/http"
	"net/http/httptest"
	"strings"
	"testing"

	"github.com/miles1150/personal-plugins/tools/keyenv/internal/vault"
)

func TestVaultMarkersAccountAndHeader(t *testing.T) {
	const vid = "11111111111111111111111111111111"
	server := httptest.NewServer(http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		switch r.URL.Path {
		case "/drive/v3/about":
			if r.URL.Query().Get("fields") != "user(emailAddress)" {
				t.Error("overbroad account query")
			}
			io.WriteString(w, `{"user":{"emailAddress":"synthetic@example.invalid"}}`)
		case "/drive/v3/files/test-folder":
			io.WriteString(w, `{"mimeType":"application/vnd.google-apps.folder","appProperties":{"keyenvVaultFolder":"`+vid+`"}}`)
		case "/drive/v3/files":
			q := r.URL.Query().Get("q")
			if !strings.Contains(q, "keyenvHeader") || !strings.Contains(q, vid) || strings.Contains(q, "keyenvLab") {
				t.Error("header mixed with lab/revisions")
			}
			io.WriteString(w, `{"files":[{"id":"test-header"}]}`)
		case "/drive/v3/files/test-header":
			if r.URL.Query().Get("alt") == "media" {
				io.WriteString(w, `{"version":1}`)
			} else {
				json.NewEncoder(w).Encode(map[string]any{"parents": []string{"test-folder"}, "appProperties": map[string]string{"keyenvHeader": vid}})
			}
		default:
			t.Error("unexpected request", r.URL.Path)
			w.WriteHeader(400)
		}
	}))
	defer server.Close()
	s := Store{Client: server.Client(), BaseURL: server.URL, FolderID: "test-folder", Marker: "keyenvRevision", Value: vid}
	ctx := context.Background()
	account, e := s.Account(ctx)
	if e != nil || account != "synthetic@example.invalid" {
		t.Fatal(e)
	}
	id, e := s.VaultFolder(ctx)
	if e != nil || id != vid {
		t.Fatal(e)
	}
	hid, b, e := s.ReadHeader(ctx, vid)
	if e != nil || hid != "test-header" || string(b) != `{"version":1}` {
		t.Fatal(e)
	}
}

func TestVaultFolderRejectsLabAndMultipleHeaders(t *testing.T) {
	for _, metadata := range []string{`{"mimeType":"application/vnd.google-apps.folder","appProperties":{"keyenvLabFolder":"1"}}`, `{"mimeType":"application/vnd.google-apps.folder","appProperties":{"keyenvVaultFolder":"valid-vault-id"},"trashed":true}`} {
		server := httptest.NewServer(http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) { io.WriteString(w, metadata) }))
		s := Store{Client: server.Client(), BaseURL: server.URL, FolderID: "test-folder"}
		if _, e := s.VaultFolder(context.Background()); !errors.Is(e, vault.ErrIntegrity) {
			t.Fatal("non-vault accepted", e)
		}
		server.Close()
	}
	server := httptest.NewServer(http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		io.WriteString(w, `{"files":[{"id":"header-one"},{"id":"header-two"}]}`)
	}))
	defer server.Close()
	s := Store{Client: server.Client(), BaseURL: server.URL, FolderID: "test-folder"}
	if _, _, e := s.ReadHeader(context.Background(), "valid-vault-id"); !errors.Is(e, vault.ErrIntegrity) {
		t.Fatal("ambiguous header accepted", e)
	}
}

func TestVaultFolderSameIDRecovery(t *testing.T) {
	const id = "reserved-folder"
	const vid = "11111111111111111111111111111111"
	created := false
	posts := 0
	server := httptest.NewServer(http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		if r.Method == http.MethodPost {
			posts++
			var metadata struct {
				ID    string            `json:"id"`
				Props map[string]string `json:"appProperties"`
			}
			json.NewDecoder(r.Body).Decode(&metadata)
			if metadata.ID != id || metadata.Props["keyenvVaultFolder"] != vid {
				t.Error("unsaved/random replacement ID used")
			}
			if created {
				w.WriteHeader(409)
				return
			}
			created = true
			w.WriteHeader(503)
			return // Saved remotely; reply lost.
		}
		if !created || r.URL.Path != "/drive/v3/files/"+id {
			t.Error("wrong recovery query")
		}
		io.WriteString(w, `{"mimeType":"application/vnd.google-apps.folder","appProperties":{"keyenvVaultFolder":"`+vid+`"}}`)
	}))
	defer server.Close()
	s := Store{Client: server.Client(), BaseURL: server.URL}
	if e := s.CreateVaultFolderAt(context.Background(), id, vid); !errors.Is(e, vault.ErrUnavailable) {
		t.Fatal(e)
	}
	if e := s.CreateVaultFolderAt(context.Background(), id, vid); e != nil {
		t.Fatal("same-ID recovery failed", e)
	}
	if !created || posts != 2 {
		t.Fatal("unexpected create/retry behavior")
	}
}
