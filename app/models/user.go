package models

import (
	"time"

	"github.com/google/uuid"
)

type User struct {
	Id uuid.UUID `json:"id,omitempty"`
	Name string `json:"name"`
	Email string `json:"email"`
	PasswordHash string `json:"password_hash"`
	CreatedAt time.Time `json:"created_at"`
	UpdatedAt time.Time `json:"updated_at"`
	DeletedAt time.Time `json:"deleted_at"`
}

func NewUser(name, email, password string) User {
	return User{
		Name: name,
		Email: email,
		PasswordHash: password,
	}
}
