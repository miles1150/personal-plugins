package drive

import (
	"context"
	"crypto/subtle"
	"encoding/json"
	"fmt"
	"io"
	"net"
	"net/http"
	"os"
	"time"

	"github.com/miles1150/personal-plugins/tools/keyenv/internal/vault"
	"golang.org/x/oauth2"
)

const DriveFileScope = "https://www.googleapis.com/auth/drive.file"

// AuthorizeLab deliberately obtains in-memory credentials only.
func AuthorizeLab(ctx context.Context, clientPath string, instructions io.Writer) (*http.Client, error) {
	return Authorize(ctx, clientPath, "", instructions)
}

func ClientIdentity(clientPath string) (string, error) {
	if !privateClientFile(clientPath) {
		return "", vault.ErrAuthRequired
	}
	f, e := os.Open(clientPath)
	if e != nil {
		return "", vault.ErrAuthRequired
	}
	defer f.Close()
	b, e := io.ReadAll(io.LimitReader(f, 65537))
	defer clear(b)
	if e != nil || len(b) > 65536 {
		return "", vault.ErrInvalid
	}
	var c struct {
		Installed struct {
			ID string `json:"client_id"`
		} `json:"installed"`
	}
	if json.Unmarshal(b, &c) != nil || c.Installed.ID == "" {
		return "", vault.ErrInvalid
	}
	return c.Installed.ID, nil
}

// Authorize requests only drive.file; credentials remain in this process.
func Authorize(ctx context.Context, clientPath, account string, instructions io.Writer) (*http.Client, error) {
	ctx = secureOAuthContext(ctx)
	cfg, err := readOAuthConfig(clientPath)
	if err != nil {
		return nil, err
	}
	token, err := browserAuthorize(ctx, cfg, account, instructions, false)
	if err != nil {
		return nil, err
	}
	return oauthClient(ctx, cfg.TokenSource(ctx, token)), nil
}

func readOAuthConfig(clientPath string) (*oauth2.Config, error) {
	if !privateClientFile(clientPath) {
		return nil, vault.ErrAuthRequired
	}
	f, err := os.Open(clientPath)
	if err != nil {
		return nil, vault.ErrAuthRequired
	}
	defer f.Close()
	data, err := io.ReadAll(io.LimitReader(f, 65537))
	if err != nil || len(data) > 65536 {
		return nil, vault.ErrInvalid
	}
	var config struct {
		Installed struct {
			ID     string `json:"client_id"`
			Secret string `json:"client_secret"`
		} `json:"installed"`
	}
	if json.Unmarshal(data, &config) != nil || config.Installed.ID == "" {
		return nil, vault.ErrInvalid
	}
	clear(data)
	return &oauth2.Config{ClientID: config.Installed.ID, ClientSecret: config.Installed.Secret, Endpoint: oauth2.Endpoint{AuthURL: "https://accounts.google.com/o/oauth2/v2/auth", TokenURL: "https://oauth2.googleapis.com/token", AuthStyle: oauth2.AuthStyleInParams}, Scopes: []string{DriveFileScope}}, nil
}

func browserAuthorize(ctx context.Context, original *oauth2.Config, account string, instructions io.Writer, persistent bool) (*oauth2.Token, error) {
	ctx = secureOAuthContext(ctx)
	l, err := net.Listen("tcp", "127.0.0.1:0")
	if err != nil {
		return nil, vault.ErrAuthRequired
	}
	defer l.Close()
	cfg := *original
	cfg.RedirectURL = "http://" + l.Addr().String() + "/callback"
	state, verifier := oauth2.GenerateVerifier(), oauth2.GenerateVerifier()
	result := make(chan string, 1)
	mux := http.NewServeMux()
	mux.HandleFunc("/callback", func(w http.ResponseWriter, r *http.Request) {
		if r.Method != http.MethodGet || subtle.ConstantTimeCompare([]byte(r.URL.Query().Get("state")), []byte(state)) != 1 {
			http.Error(w, "Invalid callback", http.StatusBadRequest)
			return
		}
		code := r.URL.Query().Get("code")
		if len(code) > 8192 {
			http.Error(w, "Invalid callback", http.StatusBadRequest)
			return
		}
		if code == "" && r.URL.Query().Get("error") == "" {
			http.Error(w, "Missing code", http.StatusBadRequest)
			return
		}
		w.Header().Set("Cache-Control", "no-store")
		select {
		case result <- code:
			fmt.Fprintln(w, "Return to the terminal to see the result.")
		default:
			http.Error(w, "Already received", http.StatusConflict)
		}
	})
	server := &http.Server{Handler: mux, ReadHeaderTimeout: 5 * time.Second, ReadTimeout: 10 * time.Second, WriteTimeout: 10 * time.Second, MaxHeaderBytes: 16 * 1024}
	defer server.Close()
	go func() { _ = server.Serve(l) }()
	if persistent {
		fmt.Fprintln(instructions, "请本人在浏览器授权现有应用：仅 drive.file；登录令牌将加密保存在本机，后续自动刷新，不使用钥匙串。")
	} else {
		fmt.Fprintln(instructions, "请在本机浏览器打开以下地址。本次仅请求 drive.file，登录材料只保留在当前进程中：")
	}
	options := []oauth2.AuthCodeOption{oauth2.AccessTypeOnline, oauth2.S256ChallengeOption(verifier), oauth2.SetAuthURLParam("prompt", "select_account")}
	if persistent {
		options = []oauth2.AuthCodeOption{oauth2.AccessTypeOffline, oauth2.S256ChallengeOption(verifier), oauth2.SetAuthURLParam("prompt", "consent select_account")}
	}
	if account != "" {
		options = append(options, oauth2.SetAuthURLParam("login_hint", account))
	}
	fmt.Fprintln(instructions, cfg.AuthCodeURL(state, options...))
	select {
	case <-ctx.Done():
		return nil, vault.ErrAuthRequired
	case code := <-result:
		if code == "" {
			return nil, vault.ErrAuthRequired
		}
		token, err := cfg.Exchange(ctx, code, oauth2.VerifierOption(verifier))
		if err != nil {
			return nil, vault.ErrAuthRequired
		}
		return token, nil
	}
}

func oauthClient(ctx context.Context, source oauth2.TokenSource) *http.Client {
	client := oauth2.NewClient(ctx, source)
	client.Timeout = 30 * time.Second
	client.CheckRedirect = func(*http.Request, []*http.Request) error { return http.ErrUseLastResponse }
	return client
}

func privateClientFile(path string) bool {
	info, err := os.Lstat(path)
	return err == nil && info.Mode().IsRegular() && info.Mode().Perm()&0077 == 0 && info.Size() <= 65536
}

// This client is also used by the token endpoint: never redirect credential
// POSTs, and bound refresh/exchange network waits independently of consent time.
func secureOAuthContext(ctx context.Context) context.Context {
	base, _ := ctx.Value(oauth2.HTTPClient).(*http.Client)
	if base == nil {
		base = http.DefaultClient
	}
	client := *base
	client.Timeout = 30 * time.Second
	client.CheckRedirect = func(*http.Request, []*http.Request) error { return http.ErrUseLastResponse }
	return context.WithValue(ctx, oauth2.HTTPClient, &client)
}
