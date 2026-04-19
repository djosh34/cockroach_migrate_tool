package typeconv

import "fmt"

type ShortDesc = string

const (
	InvalidDecimalArgs       ShortDesc = "Invalid decimal args"
	UnsupportedBytesMax      ShortDesc = "Bytes limit not supported"
	UnsupportedColumnTypeRaw ShortDesc = "Unsupported column type"
	UnsupportedTinyInt       ShortDesc = "TINYINT not supported"
	UnknownTypeForOID        ShortDesc = "Unknown OID to convert to type"
	OIDNotFoundForTypeFromPG ShortDesc = "Failed finding OID for a type from PG source"
	UnknownIntervalType      ShortDesc = "Unsupported interval type"
)

var (
	UnsupportedCollate    = func(c string) ShortDesc { return fmt.Sprintf("collate %s not supported", c) }
	UnsupportedColumnType = func(c string) ShortDesc { return fmt.Sprintf("%s %s", UnsupportedColumnTypeRaw, c) }
)
