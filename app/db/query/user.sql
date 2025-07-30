-- name: CreateUser :one
INSERT INTO users (
    username,
    email,
    password_hash
) VALUES (
    $1, $2, $3
)
RETURNING id, username, email, created_at, updated_at;

-- name: GetUser :one
SELECT id, username, email, created_at, updated_at
FROM users
WHERE id = $1 AND deleted_at IS NULL
LIMIT 1;

-- -- name: ListUsers :many
-- SELECT id, username, email, created_at, updated_at
-- FROM users
-- WHERE deleted_at IS NULL
-- ORDER BY created_at DESC;

-- name: UpdateUser :one
UPDATE users
SET
    username = COALESCE(sqlc.narg('username'), username), -- COALESCE return the first non-nil value, if no "username" argument is provided will keep the current username
    email = COALESCE(sqlc.narg('email'), email),
    password_hash = COALESCE(sqlc.narg('password_hash'), password_hash),
    updated_at = NOW()
WHERE
    id = sqlc.arg('id') AND deleted_at IS NULL
RETURNING id, username, email, created_at, updated_at;

-- name: SoftDeleteUser :exec
UPDATE users
SET deleted_at = NOW()
WHERE id = $1 AND deleted_at IS NULL;

-- -- name: HardDeleteUser :exec
-- DELETE FROM users
-- WHERE id = $1;
