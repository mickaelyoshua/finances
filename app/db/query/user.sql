-- name: CreateUser :one
INSERT INTO users (
    name,
    email,
    password_hash
) VALUES (
    $1, $2, $3
)
RETURNING id, name, email, password_hash, created_at, updated_at;

-- name: GetUserById :one
SELECT id, name, email, password_hash, created_at, updated_at
FROM users
WHERE id = $1 AND deleted_at IS NULL
LIMIT 1;

-- name: GetUserByEmail :one
SELECT id, name, email, password_hash, created_at, updated_at
FROM users
WHERE email = $1 AND deleted_at IS NULL
LIMIT 1;


-- name: SearchEmail :one
SELECT email
FROM users
WHERE email = $1 AND deleted_at IS NULL;

-- -- name: ListUsers :many
-- SELECT id, username, email, created_at, updated_at
-- FROM users
-- WHERE deleted_at IS NULL
-- ORDER BY created_at DESC;

-- name: UpdateUser :one
UPDATE users
SET
    name = COALESCE(sqlc.narg('name'), name), -- COALESCE return the first non-nil value, if no "name" argument is provided will keep the current name
    email = COALESCE(sqlc.narg('email'), email),
    password_hash = COALESCE(sqlc.narg('password_hash'), password_hash),
    updated_at = NOW()
WHERE
    id = sqlc.arg('id') AND deleted_at IS NULL
RETURNING id, name, email, created_at, updated_at;

-- name: SoftDeleteUser :exec
UPDATE users
SET deleted_at = NOW()
WHERE id = $1 AND deleted_at IS NULL;

-- -- name: HardDeleteUser :exec
-- DELETE FROM users
-- WHERE id = $1;
