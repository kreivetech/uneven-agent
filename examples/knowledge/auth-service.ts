/**
 * AuthService — Handles user authentication
 *
 * Responsible for login, registration, and token validation.
 * Uses bcrypt for password hashing and JWT for session tokens.
 */

import bcrypt from 'bcrypt'
import jwt from 'jsonwebtoken'
import { db } from '../database'

const SALT_ROUNDS = 12
const TOKEN_EXPIRY = '24h'

export interface User {
  id: string
  email: string
  name: string
  role: 'user' | 'admin'
  createdAt: Date
}

export interface LoginResult {
  user: User
  token: string
}

export class AuthService {
  /**
   * Login user with email and password.
   * Returns user data + JWT token on success.
   * Throws AuthError on invalid credentials.
   */
  async login(email: string, password: string): Promise<LoginResult> {
    const user = await db('users').where({ email }).first()

    if (!user) {
      throw new Error('Invalid email or password')
    }

    const passwordMatch = await bcrypt.compare(password, user.password_hash)

    if (!passwordMatch) {
      throw new Error('Invalid email or password')
    }

    const token = jwt.sign(
      { userId: user.id, role: user.role },
      process.env.JWT_SECRET!,
      { expiresIn: TOKEN_EXPIRY }
    )

    return {
      user: {
        id: user.id,
        email: user.email,
        name: user.name,
        role: user.role,
        createdAt: user.created_at,
      },
      token,
    }
  }

  /**
   * Register new user.
   * Hashes password before storing.
   * Throws if email already in use.
   */
  async register(email: string, password: string, name: string): Promise<User> {
    const existing = await db('users').where({ email }).first()

    if (existing) {
      throw new Error('Email already registered')
    }

    const passwordHash = await bcrypt.hash(password, SALT_ROUNDS)

    const [user] = await db('users')
      .insert({ email, password_hash: passwordHash, name, role: 'user' })
      .returning(['id', 'email', 'name', 'role', 'created_at'])

    return {
      id: user.id,
      email: user.email,
      name: user.name,
      role: user.role,
      createdAt: user.created_at,
    }
  }

  /**
   * Validate JWT token and return decoded payload.
   * Throws if token is invalid or expired.
   */
  validateToken(token: string): { userId: string; role: string } {
    try {
      const payload = jwt.verify(token, process.env.JWT_SECRET!) as any
      return { userId: payload.userId, role: payload.role }
    } catch {
      throw new Error('Invalid or expired token')
    }
  }
}
