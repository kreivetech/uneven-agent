# Known Bugs & Issues

## BUG-001: JWT token not invalidated on logout

**Severity:** High
**Status:** Open

### Description
When a user calls `POST /auth/logout`, the server returns 200 OK but the JWT token
remains valid until its natural expiry (24h). If the token is stolen, there is no
way to revoke access.

### Root Cause
The current implementation does not maintain a token blacklist or use short-lived tokens.
`AuthService.validateToken()` only checks signature and expiry — no revocation check.

### Fix
Implement a token blacklist in Redis:
1. On logout, store `token_hash` in Redis with TTL = token expiry
2. In `validateToken()`, check Redis blacklist before returning payload
3. Alternatively, switch to short-lived access tokens (15min) + refresh tokens

---

## BUG-002: SQL injection risk in task search

**Severity:** Critical
**Status:** Open

### Description
The `GET /tasks?search=` query parameter is passed directly into a raw SQL LIKE clause
without sanitization:

```typescript
// DANGEROUS — never do this
db.raw(`SELECT * FROM tasks WHERE title LIKE '%${search}%'`)
```

### Fix
Use parameterized queries:
```typescript
db('tasks').where('title', 'like', `%${search}%`)
```

---

## BUG-003: Race condition in task completion

**Severity:** Medium
**Status:** In Progress

### Description
When two requests call `PATCH /tasks/:id/complete` simultaneously, both can succeed
and the `completed_at` timestamp may be set twice with different values.

### Fix
Use a database transaction with SELECT FOR UPDATE to lock the row before updating.

---

## BUG-004: Missing rate limiting on /auth/login

**Severity:** High
**Status:** Open

### Description
The login endpoint has no rate limiting, making it vulnerable to brute-force attacks.
An attacker can try unlimited password combinations.

### Fix
Add rate limiting middleware (e.g., express-rate-limit):
- Max 5 attempts per IP per 15 minutes
- Return 429 Too Many Requests when exceeded
- Log failed attempts for monitoring

---

## BUG-005: JWT_SECRET not validated at startup

**Severity:** High
**Status:** Open

### Description
`AuthService.login()` calls `jwt.sign(payload, process.env.JWT_SECRET!)` without first
checking that `JWT_SECRET` is defined. If the environment variable is missing, the runtime
error thrown by `jsonwebtoken` is `Error: secretOrPrivateKey must have a value` — which gives
no hint that a missing env var is the root cause. In misconfigured deployments this surfaces
as a cryptic 500 error on every login attempt.

### Fix
Add an explicit guard at startup (or in the constructor/module init):
```typescript
if (!process.env.JWT_SECRET) {
  throw new Error('JWT_SECRET environment variable is required but not set')
}
```

---

## BUG-006: No password validation in register — empty passwords accepted

**Severity:** Medium
**Status:** Open

### Description
`AuthService.register()` passes the raw `password` string directly to `bcrypt.hash()`
without any validation. An empty string (`""`) is hashed successfully, and `bcrypt.compare("", hash)`
returns `true` — meaning a user can register and authenticate with no password at all.

### Fix
Validate password before hashing:
```typescript
if (!password || password.length < 8) {
  throw new Error('Password must be at least 8 characters')
}
```

---

## BUG-007: Timing side-channel in login() enables email enumeration

**Severity:** Medium
**Status:** Open

### Description
`AuthService.login()` returns immediately (~0ms) when the email is not found, but waits
for `bcrypt.compare()` (~200–300ms) when the email exists but the password is wrong.
An attacker can distinguish valid from invalid emails by measuring response time —
`valid email + wrong password` takes ~300ms; `invalid email` takes ~0ms.

### Fix
Always run a bcrypt comparison regardless of whether the user exists, using a dummy
hash for the not-found case to equalise response time:

```typescript
const DUMMY_HASH = '$2b$12$invalid.hash.to.prevent.timing.attacks.padding'

const user = await db('users').where({ email }).first()
const hashToCompare = user ? user.password_hash : DUMMY_HASH
const passwordMatch = await bcrypt.compare(password, hashToCompare)

if (!user || !passwordMatch) {
  throw new Error('Invalid email or password')
}
```
