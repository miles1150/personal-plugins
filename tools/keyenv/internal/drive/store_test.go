package drive

import (
	"bytes"
	"context"
	"encoding/json"
	"errors"
	"io"
	"mime"
	"mime/multipart"
	"net/http"
	"net/http/httptest"
	"strings"
	"testing"

	"github.com/miles1150/personal-plugins/tools/keyenv/internal/vault"
)

func TestHTTPImmutableUploadAndPagination(t *testing.T) {
	const folder = "lab-folder-123"
	objects := map[string][]byte{}
	posts := 0
	lists := 0
	server := httptest.NewServer(http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		switch {
		case r.URL.Path == "/drive/v3/files/generateIds":
			_ = json.NewEncoder(w).Encode(map[string]any{"ids": []string{"generated-file-123"}})
		case r.URL.Path == "/upload/drive/v3/files":
			posts++
			if r.Method != "POST" || r.URL.Query().Get("uploadType") != "multipart" {
				t.Error("wrong upload method")
			}
			kind, p, err := mime.ParseMediaType(r.Header.Get("Content-Type"))
			if err != nil || kind != "multipart/related" {
				t.Error("wrong upload content type")
				w.WriteHeader(400)
				return
			}
			reader := multipart.NewReader(r.Body, p["boundary"])
			metadata, err := reader.NextPart()
			if err != nil {
				t.Error(err)
				return
			}
			var m struct {
				ID         string            `json:"id"`
				Parents    []string          `json:"parents"`
				Properties map[string]string `json:"appProperties"`
			}
			if err := json.NewDecoder(metadata).Decode(&m); err != nil {
				t.Error(err)
				return
			}
			if len(m.Parents) != 1 || m.Parents[0] != folder || m.Properties["keyenvLab"] != "1" {
				t.Error("missing scope metadata")
			}
			part, err := reader.NextPart()
			if err != nil {
				t.Error(err)
				return
			}
			b, _ := io.ReadAll(part)
			if _, ok := objects[m.ID]; ok {
				w.WriteHeader(409)
				return
			}
			objects[m.ID] = b
			_ = json.NewEncoder(w).Encode(map[string]string{"id": m.ID})
		case r.URL.Path == "/drive/v3/files":
			lists++
			if !strings.Contains(r.URL.Query().Get("q"), folder) {
				t.Error("unscoped list")
			}
			if r.URL.Query().Get("pageToken") == "" {
				io.WriteString(w, `{"nextPageToken":"second-page","files":[{"id":"generated-file-123"}]}`)
			} else {
				io.WriteString(w, `{"files":[{"id":"another-file-456"}]}`)
			}
		case strings.HasPrefix(r.URL.Path, "/drive/v3/files/"):
			id := strings.TrimPrefix(r.URL.Path, "/drive/v3/files/")
			b, ok := objects[id]
			if !ok {
				w.WriteHeader(404)
				return
			}
			if r.URL.Query().Get("alt") == "media" {
				_, _ = w.Write(b)
			} else {
				_ = json.NewEncoder(w).Encode(map[string]any{"parents": []string{folder}, "appProperties": map[string]string{"keyenvLab": "1"}})
			}
		default:
			w.WriteHeader(404)
		}
	}))
	defer server.Close()
	s := &Store{Client: server.Client(), BaseURL: server.URL, FolderID: folder}
	ctx := context.Background()
	id, err := s.Reserve(ctx)
	if err != nil {
		t.Fatal(err)
	}
	data := []byte("synthetic-encrypted-bytes")
	if err := s.Put(ctx, id, data); err != nil {
		t.Fatal(err)
	}
	if err := s.Put(ctx, id, data); err != nil {
		t.Fatal("retry failed", err)
	}
	if posts != 2 || len(objects) != 1 {
		t.Fatal("retry duplicated object")
	}
	if err := s.Put(ctx, id, []byte("different")); !errors.Is(err, vault.ErrIntegrity) {
		t.Fatal("409 accepted different bytes", err)
	}
	actual, err := s.Get(ctx, id)
	if err != nil || !bytes.Equal(actual, data) {
		t.Fatal(err)
	}
	ids, err := s.List(ctx)
	if err != nil || len(ids) != 2 || lists != 2 {
		t.Fatal("pagination failed", err)
	}
}

func TestListFailureNeverReturnsPartialSuccess(t *testing.T) {
	for _, tc := range []struct {
		name     string
		second   string
		status   int
		expected error
	}{
		{"failed-page", "", 503, vault.ErrUnavailable},
		{"repeated-token", `{"nextPageToken":"next","files":[]}`, 200, vault.ErrInvalid},
		{"incomplete", `{"incompleteSearch":true,"files":[]}`, 200, ErrRemote},
		{"duplicate-id", `{"files":[{"id":"first-file-123"}]}`, 200, vault.ErrInvalid},
	} {
		t.Run(tc.name, func(t *testing.T) {
			server := httptest.NewServer(http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
				if r.URL.Query().Get("pageToken") == "" {
					io.WriteString(w, `{"nextPageToken":"next","files":[{"id":"first-file-123"}]}`)
					return
				}
				w.WriteHeader(tc.status)
				io.WriteString(w, tc.second)
			}))
			defer server.Close()
			s := Store{Client: server.Client(), BaseURL: server.URL, FolderID: "test-folder"}
			ids, err := s.List(context.Background())
			if !errors.Is(err, tc.expected) || ids != nil {
				t.Fatal("partial listing treated as success", ids, err)
			}
		})
	}
}

func TestHTTPFailureClassificationAndForeignObject(t *testing.T) {
	for _, tc := range []struct {
		status int
		want   error
	}{{401, vault.ErrAuthRequired}, {403, vault.ErrAuthRequired}, {404, vault.ErrIntegrity}, {429, vault.ErrUnavailable}, {500, vault.ErrUnavailable}, {400, ErrRemote}} {
		server := httptest.NewServer(http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
			w.WriteHeader(tc.status)
			io.WriteString(w, "sensitive-server-error-body")
		}))
		s := Store{Client: server.Client(), BaseURL: server.URL, FolderID: "test-folder"}
		_, err := s.List(context.Background())
		if !errors.Is(err, tc.want) || strings.Contains(err.Error(), "sensitive") {
			t.Error("unsafe classification", tc.status, err)
		}
		server.Close()
	}
	server := httptest.NewServer(http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		if r.URL.Query().Get("alt") == "media" {
			t.Error("read foreign payload")
		}
		io.WriteString(w, `{"parents":["other-folder"],"appProperties":{"keyenvLab":"1"}}`)
	}))
	defer server.Close()
	s := Store{Client: server.Client(), BaseURL: server.URL, FolderID: "test-folder"}
	if _, err := s.Get(context.Background(), "some-file-id"); !errors.Is(err, vault.ErrIntegrity) {
		t.Fatal("foreign object accepted")
	}
}

func TestOversizedResponseRejected(t *testing.T) {
	server := httptest.NewServer(http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		_, _ = w.Write(bytes.Repeat([]byte("x"), vault.MaxObject+1))
	}))
	defer server.Close()
	s := Store{Client: server.Client(), BaseURL: server.URL, FolderID: "test-folder"}
	if _, err := s.List(context.Background()); !errors.Is(err, vault.ErrInvalid) {
		t.Fatal("oversized response accepted", err)
	}
}

func TestCheckLabFolderBoundary(t *testing.T) {
	for _, tc := range []struct {
		name     string
		metadata string
		want     error
	}{
		{"lab-folder", `{"mimeType":"application/vnd.google-apps.folder","appProperties":{"keyenvLabFolder":"1"},"trashed":false}`, nil},
		{"ordinary-folder", `{"mimeType":"application/vnd.google-apps.folder","appProperties":{},"trashed":false}`, vault.ErrIntegrity},
		{"wrong-marker", `{"mimeType":"application/vnd.google-apps.folder","appProperties":{"keyenvLabFolder":"0"},"trashed":false}`, vault.ErrIntegrity},
		{"marked-file", `{"mimeType":"application/octet-stream","appProperties":{"keyenvLabFolder":"1"},"trashed":false}`, vault.ErrIntegrity},
		{"trashed-folder", `{"mimeType":"application/vnd.google-apps.folder","appProperties":{"keyenvLabFolder":"1"},"trashed":true}`, vault.ErrIntegrity},
		{"malformed", `{`, vault.ErrIntegrity},
	} {
		t.Run(tc.name, func(t *testing.T) {
			server := httptest.NewServer(http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
				if r.Method != http.MethodGet || r.URL.Path != "/drive/v3/files/test-folder" || r.URL.Query().Get("fields") != "mimeType,appProperties,trashed" {
					t.Error("folder validation must only read the selected folder metadata")
				}
				io.WriteString(w, tc.metadata)
			}))
			defer server.Close()
			s := Store{Client: server.Client(), BaseURL: server.URL, FolderID: "test-folder"}
			if err := s.CheckLabFolder(context.Background()); !errors.Is(err, tc.want) {
				t.Fatalf("folder boundary: got %v, want %v", err, tc.want)
			}
		})
	}
}
