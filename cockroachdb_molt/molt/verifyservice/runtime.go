package verifyservice

import (
	"context"
	"crypto/tls"
	"crypto/x509"
	"net/http"
	"os"
	"time"

	"github.com/cockroachdb/errors"
	"github.com/rs/zerolog"
)

type RuntimeDependencies struct {
	Runner      Runner
	IDGenerator func() string
	Now         func() time.Time
	Logger      zerolog.Logger
}

func Run(ctx context.Context, cfg Config, deps RuntimeDependencies) error {
	runner := deps.Runner
	if runner == nil {
		runner = NewVerifyRunner(cfg, deps.Logger)
	}

	service := NewService(cfg, Dependencies{
		Runner:      runner,
		IDGenerator: deps.IDGenerator,
		Now:         deps.Now,
	})
	defer service.Close()

	server := &http.Server{
		Addr:    cfg.Listener.BindAddr,
		Handler: service.Handler(),
	}
	tlsConfig, err := cfg.Listener.TLS.ServerTLSConfig()
	if err != nil {
		return err
	}
	server.TLSConfig = tlsConfig

	shutdownErrCh := make(chan error, 1)
	go func() {
		<-ctx.Done()
		service.Close()

		shutdownCtx, cancel := context.WithTimeout(context.Background(), 5*time.Second)
		defer cancel()
		shutdownErrCh <- server.Shutdown(shutdownCtx)
	}()

	err = server.ListenAndServeTLS("", "")
	if !errors.Is(err, http.ErrServerClosed) {
		return err
	}

	select {
	case shutdownErr := <-shutdownErrCh:
		return shutdownErr
	default:
		return nil
	}
}

func (cfg ListenerTLSConfig) ServerTLSConfig() (*tls.Config, error) {
	tlsConfig := &tls.Config{
		MinVersion: tls.VersionTLS12,
	}

	serverCertificate, err := tls.LoadX509KeyPair(cfg.CertPath, cfg.KeyPath)
	if err != nil {
		return nil, err
	}
	tlsConfig.Certificates = []tls.Certificate{serverCertificate}

	if cfg.ClientAuth.Mode != ListenerClientAuthModeMTLS {
		return tlsConfig, nil
	}

	clientCAContents, err := os.ReadFile(cfg.ClientAuth.ClientCAPath)
	if err != nil {
		return nil, err
	}

	clientCAPool := x509.NewCertPool()
	if !clientCAPool.AppendCertsFromPEM(clientCAContents) {
		return nil, errors.New("parse client ca certificate")
	}

	tlsConfig.ClientAuth = tls.RequireAndVerifyClientCert
	tlsConfig.ClientCAs = clientCAPool
	return tlsConfig, nil
}
