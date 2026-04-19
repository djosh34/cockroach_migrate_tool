package verifyservice

import (
	"bytes"
	"context"
	"crypto/rand"
	"crypto/rsa"
	"crypto/tls"
	"crypto/x509"
	"crypto/x509/pkix"
	"encoding/pem"
	"math/big"
	"net"
	"net/http"
	"os"
	"path/filepath"
	"syscall"
	"testing"
	"time"

	"github.com/cockroachdb/molt/verify/inconsistency"
	"github.com/rs/zerolog"
	"github.com/stretchr/testify/require"
)

func TestServerTLSConfigLoadsServerCertificateAndEnforcesMTLS(t *testing.T) {
	tlsFiles := writeListenerTLSFiles(t)

	tlsConfig, err := (ListenerTLSConfig{
		CertPath: tlsFiles.serverCertPath,
		KeyPath:  tlsFiles.serverKeyPath,
		ClientAuth: ListenerClientAuthConfig{
			Mode:         ListenerClientAuthModeMTLS,
			ClientCAPath: tlsFiles.clientCAPath,
		},
	}).ServerTLSConfig()

	require.NoError(t, err)
	require.Equal(t, uint16(tls.VersionTLS12), tlsConfig.MinVersion)
	require.Equal(t, tls.RequireAndVerifyClientCert, tlsConfig.ClientAuth)
	require.Len(t, tlsConfig.Certificates, 1)
	require.NotNil(t, tlsConfig.ClientCAs)

	loadedServerCert, err := x509.ParseCertificate(tlsConfig.Certificates[0].Certificate[0])
	require.NoError(t, err)
	require.Equal(t, tlsFiles.serverCert.Subject.CommonName, loadedServerCert.Subject.CommonName)

	require.True(t, certPoolContainsSubject(tlsConfig.ClientCAs, tlsFiles.clientCACert.RawSubject))
}

func TestRunServesHTTPSUsingPreloadedServerTLSConfig(t *testing.T) {
	tlsFiles := writeListenerTLSFiles(t)
	bindAddr := reserveLocalAddress(t)
	clientCAFIFOPath := filepath.Join(t.TempDir(), "client-ca.fifo")
	require.NoError(t, syscall.Mkfifo(clientCAFIFOPath, 0o600))

	ctx, cancel := context.WithCancel(context.Background())
	t.Cleanup(cancel)

	runErrCh := make(chan error, 1)
	go func() {
		runErrCh <- Run(ctx, Config{
			Listener: ListenerConfig{
				BindAddr: bindAddr,
				Transport: ListenerTransportConfig{
					Mode: ListenerTransportModeHTTPS,
				},
				TLS: ListenerTLSConfig{
					CertPath: tlsFiles.serverCertPath,
					KeyPath:  tlsFiles.serverKeyPath,
					ClientAuth: ListenerClientAuthConfig{
						Mode:         ListenerClientAuthModeMTLS,
						ClientCAPath: clientCAFIFOPath,
					},
				},
			},
		}, RuntimeDependencies{
			Runner: noopRunner{},
			Logger: zerolog.Nop(),
		})
	}()

	type fifoOpenResult struct {
		file *os.File
		err  error
	}

	clientCAWriterReady := make(chan fifoOpenResult, 1)
	go func() {
		clientCAWriter, err := os.OpenFile(clientCAFIFOPath, os.O_WRONLY, 0o600)
		clientCAWriterReady <- fifoOpenResult{
			file: clientCAWriter,
			err:  err,
		}
	}()

	var clientCAWriter *os.File
	select {
	case result := <-clientCAWriterReady:
		require.NoError(t, result.err)
		clientCAWriter = result.file
	case <-time.After(5 * time.Second):
		t.Fatal("expected runtime to start loading the client CA")
	}
	t.Cleanup(func() {
		_ = clientCAWriter.Close()
	})

	require.NoError(t, os.Remove(tlsFiles.serverCertPath))
	require.NoError(t, os.Remove(tlsFiles.serverKeyPath))
	_, err := clientCAWriter.Write(tlsFiles.clientCAPEM)
	require.NoError(t, err)
	require.NoError(t, clientCAWriter.Close())

	clientCertificate, err := tls.LoadX509KeyPair(tlsFiles.clientCertPath, tlsFiles.clientKeyPath)
	require.NoError(t, err)

	rootCAs := x509.NewCertPool()
	require.True(t, rootCAs.AppendCertsFromPEM(tlsFiles.clientCAPEM))

	client := &http.Client{
		Transport: &http.Transport{
			TLSClientConfig: &tls.Config{
				MinVersion:   tls.VersionTLS12,
				RootCAs:      rootCAs,
				Certificates: []tls.Certificate{clientCertificate},
			},
		},
		Timeout: 2 * time.Second,
	}

	deadline := time.Now().Add(5 * time.Second)
	for time.Now().Before(deadline) {
		select {
		case err := <-runErrCh:
			require.NoError(t, err)
			t.Fatal("runtime exited before serving HTTPS")
		default:
		}

		response, err := client.Get("https://" + bindAddr + "/metrics")
		if err == nil {
			defer func() {
				_ = response.Body.Close()
			}()
			if response.StatusCode == http.StatusOK {
				cancel()
				select {
				case err := <-runErrCh:
					require.NoError(t, err)
					return
				case <-time.After(5 * time.Second):
					t.Fatal("expected runtime to shut down")
				}
			}
		}
		time.Sleep(25 * time.Millisecond)
	}

	t.Fatal("expected runtime to serve metrics over mTLS")
}

type listenerTLSFiles struct {
	serverCertPath string
	serverKeyPath  string
	clientCAPath   string
	clientCertPath string
	clientKeyPath  string
	clientCAPEM    []byte
	serverCert     *x509.Certificate
	clientCACert   *x509.Certificate
}

func writeListenerTLSFiles(t *testing.T) listenerTLSFiles {
	t.Helper()

	tempDir := t.TempDir()

	clientCA := createCertificateAuthority(t, "verify-client-ca")
	serverCertPEM, serverKeyPEM, serverCert := createSignedCertificate(t, clientCA, certificateSpec{
		commonName: "verify.internal",
		dnsNames:   []string{"verify.internal"},
		ipAddresses: []net.IP{
			net.ParseIP("127.0.0.1"),
		},
		extKeyUsage: []x509.ExtKeyUsage{x509.ExtKeyUsageServerAuth},
	})
	clientCertPEM, clientKeyPEM, _ := createSignedCertificate(t, clientCA, certificateSpec{
		commonName:  "verify-client",
		extKeyUsage: []x509.ExtKeyUsage{x509.ExtKeyUsageClientAuth},
	})

	clientCAPath := filepath.Join(tempDir, "client-ca.pem")
	serverCertPath := filepath.Join(tempDir, "server-cert.pem")
	serverKeyPath := filepath.Join(tempDir, "server-key.pem")
	clientCertPath := filepath.Join(tempDir, "client-cert.pem")
	clientKeyPath := filepath.Join(tempDir, "client-key.pem")

	require.NoError(t, os.WriteFile(clientCAPath, clientCA.certPEM, 0o600))
	require.NoError(t, os.WriteFile(serverCertPath, serverCertPEM, 0o600))
	require.NoError(t, os.WriteFile(serverKeyPath, serverKeyPEM, 0o600))
	require.NoError(t, os.WriteFile(clientCertPath, clientCertPEM, 0o600))
	require.NoError(t, os.WriteFile(clientKeyPath, clientKeyPEM, 0o600))

	return listenerTLSFiles{
		serverCertPath: serverCertPath,
		serverKeyPath:  serverKeyPath,
		clientCAPath:   clientCAPath,
		clientCertPath: clientCertPath,
		clientKeyPath:  clientKeyPath,
		clientCAPEM:    clientCA.certPEM,
		serverCert:     serverCert,
		clientCACert:   clientCA.certificate,
	}
}

type certificateAuthority struct {
	certPEM     []byte
	certificate *x509.Certificate
	privateKey  *rsa.PrivateKey
}

type certificateSpec struct {
	commonName  string
	dnsNames    []string
	ipAddresses []net.IP
	extKeyUsage []x509.ExtKeyUsage
}

func createCertificateAuthority(t *testing.T, commonName string) certificateAuthority {
	t.Helper()

	privateKey, err := rsa.GenerateKey(rand.Reader, 2048)
	require.NoError(t, err)

	template := &x509.Certificate{
		SerialNumber: newSerialNumber(t),
		Subject: pkix.Name{
			CommonName: commonName,
		},
		NotBefore:             time.Now().Add(-time.Minute),
		NotAfter:              time.Now().Add(time.Hour),
		KeyUsage:              x509.KeyUsageCertSign | x509.KeyUsageCRLSign,
		BasicConstraintsValid: true,
		IsCA:                  true,
	}

	certDER, err := x509.CreateCertificate(rand.Reader, template, template, &privateKey.PublicKey, privateKey)
	require.NoError(t, err)

	cert, err := x509.ParseCertificate(certDER)
	require.NoError(t, err)

	return certificateAuthority{
		certPEM:     pem.EncodeToMemory(&pem.Block{Type: "CERTIFICATE", Bytes: certDER}),
		certificate: cert,
		privateKey:  privateKey,
	}
}

func createSignedCertificate(t *testing.T, certificateAuthority certificateAuthority, spec certificateSpec) ([]byte, []byte, *x509.Certificate) {
	t.Helper()

	serverKey, err := rsa.GenerateKey(rand.Reader, 2048)
	require.NoError(t, err)

	serverTemplate := &x509.Certificate{
		SerialNumber: newSerialNumber(t),
		Subject: pkix.Name{
			CommonName: spec.commonName,
		},
		NotBefore:   time.Now().Add(-time.Minute),
		NotAfter:    time.Now().Add(time.Hour),
		KeyUsage:    x509.KeyUsageDigitalSignature | x509.KeyUsageKeyEncipherment,
		ExtKeyUsage: spec.extKeyUsage,
		DNSNames:    spec.dnsNames,
		IPAddresses: spec.ipAddresses,
	}

	serverDER, err := x509.CreateCertificate(rand.Reader, serverTemplate, certificateAuthority.certificate, &serverKey.PublicKey, certificateAuthority.privateKey)
	require.NoError(t, err)

	serverCert, err := x509.ParseCertificate(serverDER)
	require.NoError(t, err)

	return pem.EncodeToMemory(&pem.Block{Type: "CERTIFICATE", Bytes: serverDER}),
		pem.EncodeToMemory(&pem.Block{Type: "RSA PRIVATE KEY", Bytes: x509.MarshalPKCS1PrivateKey(serverKey)}),
		serverCert
}

func newSerialNumber(t *testing.T) *big.Int {
	t.Helper()

	serialNumber, err := rand.Int(rand.Reader, new(big.Int).Lsh(big.NewInt(1), 128))
	require.NoError(t, err)
	return serialNumber
}

func reserveLocalAddress(t *testing.T) string {
	t.Helper()

	listener, err := net.Listen("tcp", "127.0.0.1:0")
	require.NoError(t, err)
	defer func() {
		require.NoError(t, listener.Close())
	}()
	return listener.Addr().String()
}

func certPoolContainsSubject(certPool *x509.CertPool, expectedSubject []byte) bool {
	for _, subject := range certPool.Subjects() {
		if bytes.Equal(subject, expectedSubject) {
			return true
		}
	}
	return false
}

type noopRunner struct{}

func (noopRunner) Run(context.Context, RunRequest, inconsistency.Reporter) error {
	return nil
}
