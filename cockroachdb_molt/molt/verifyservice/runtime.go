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
	})
	defer service.Close()

	server := &http.Server{
		Addr:    cfg.Listener.BindAddr,
		Handler: service.Handler(),
	}

	shutdownErrCh := make(chan error, 1)
	go func() {
		<-ctx.Done()
		service.Close()

		shutdownCtx, cancel := context.WithTimeout(context.Background(), 5*time.Second)
		defer cancel()
		shutdownErrCh <- server.Shutdown(shutdownCtx)
	}()

	var err error
	if cfg.Listener.TLS == nil {
		err = server.ListenAndServe()
	} else {
		tlsConfig, tlsErr := cfg.Listener.TLS.ServerTLSConfig()
		if tlsErr != nil {
			return newOperatorError(
				"startup",
				"listener_tls_setup_failed",
				"verify-service listener TLS setup failed",
				operatorErrorDetail{Reason: tlsErr.Error()},
			)
		}
		server.TLSConfig = tlsConfig
		err = server.ListenAndServeTLS("", "")
	}
	if !errors.Is(err, http.ErrServerClosed) {
		return newOperatorError(
			"startup",
			"listener_start_failed",
			"verify-service listener failed to start",
			operatorErrorDetail{Reason: err.Error()},
		)
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

	if cfg.ClientCAPath == "" {
		return tlsConfig, nil
	}

	clientCAContents, err := os.ReadFile(cfg.ClientCAPath)
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
