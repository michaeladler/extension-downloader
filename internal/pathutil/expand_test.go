package pathutil

import (
	"testing"

	"github.com/stretchr/testify/assert"
)

func TestExpandUser(t *testing.T) {
	t.Parallel()
	s := ExpandUser("~/foo")
	assert.NotEqual(t, "~", s[0])
}

func TestExpandUser_IgnoreNonTilde(t *testing.T) {
	t.Parallel()
	s := "/home/foo"
	assert.Equal(t, s, ExpandUser(s))
}
