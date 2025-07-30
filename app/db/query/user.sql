-- name: CreateUser :one
INSERT INTO users (username, email, password_hash) VALUES (
	$1, $2, $3
) RETURNING *;

-- name: GetUser :one
SELECT * FROM users
WHERE id = $1 LIMIT 1;

-- name: UpdateUser :one
UPDATE users SET
	username = $2,
	email = $3,
	password_hash = $4,
	updated_at = NOW()
WHERE id = $1
RETURNING *;

-- name: DeleteUser :exec
DELETE FROM users
WHERE id = $1;
