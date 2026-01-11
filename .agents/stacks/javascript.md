# JavaScript/TypeScript Coding Standards

## Overview
- **Language**: JavaScript (ES2022+) and TypeScript (5.0+)
- **Use Cases**: Frontend UI components, build scripts, tooling, Node.js backend services
- **Official Docs**:
  - JavaScript: https://developer.mozilla.org/en-US/docs/Web/JavaScript
  - TypeScript: https://www.typescriptlang.org/docs/

## Setup and Tools

### Required Tools
- **Node.js**: v18+ or v20+ LTS recommended
- **npm** or **pnpm**: Package manager (prefer pnpm for monorepos)
- **TypeScript**: For all production code
- **Prettier**: Code formatter
- **ESLint**: Linter with TypeScript support
- **Jest** or **Vitest**: Testing framework

### Configuration Files
- **tsconfig.json**: TypeScript compiler configuration (strict mode enabled)
- **.prettierrc**: Prettier formatting configuration
- **.eslintrc.js**: ESLint rules and plugins
- **package.json**: Dependencies and scripts
- **jest.config.js** or **vitest.config.ts**: Test configuration

### Recommended tsconfig.json Settings
```json
{
  "compilerOptions": {
    "strict": true,
    "noImplicitAny": true,
    "strictNullChecks": true,
    "strictFunctionTypes": true,
    "noUnusedLocals": true,
    "noUnusedParameters": true,
    "noImplicitReturns": true,
    "noFallthroughCasesInSwitch": true,
    "esModuleInterop": true,
    "skipLibCheck": true,
    "forceConsistentCasingInFileNames": true
  }
}
```

## Coding Standards

### Naming Conventions
- **Variables and Functions**: camelCase
  - `const userName = "John"`
  - `function calculateTotal() {}`
- **Classes and Interfaces**: PascalCase
  - `class UserAccount {}`
  - `interface ApiResponse {}`
- **Constants**: UPPER_SNAKE_CASE (for true constants)
  - `const MAX_RETRIES = 3`
- **Type Aliases**: PascalCase
  - `type UserId = string`
- **Enums**: PascalCase for enum name, PascalCase for values
  - `enum UserRole { Admin, User, Guest }`
- **Files**: kebab-case
  - `user-service.ts`, `api-client.ts`
- **Test Files**: Same as source file + `.test.ts` or `.spec.ts`
  - `user-service.test.ts`

### Code Organization
- One component/class per file (exceptions for small related types)
- Group related functionality into modules/directories
- Index files (index.ts) for public API exports
- Separate concerns: logic, UI, types, tests
- Import order: external dependencies → internal modules → types → styles

**Import Order Example**:
```typescript
// External dependencies
import React from 'react';
import { useState } from 'react';

// Internal modules
import { apiClient } from '@/lib/api-client';
import { UserService } from '@/services/user-service';

// Types
import type { User, ApiResponse } from '@/types';

// Styles (if applicable)
import styles from './Component.module.css';
```

### Comments and Documentation
- **JSDoc comments** for all exported functions, classes, and interfaces
- Inline comments for complex logic only (code should be self-documenting)
- TODO comments include issue number or date: `// TODO(2026-01-11): Refactor this`
- Explain "why" not "what" in comments

**JSDoc Example**:
```typescript
/**
 * Fetches user data from the API with retry logic.
 *
 * @param userId - The unique identifier for the user
 * @param options - Optional configuration for the request
 * @returns Promise resolving to user data
 * @throws {ApiError} When the API request fails after retries
 */
async function fetchUser(userId: string, options?: RequestOptions): Promise<User> {
  // Implementation
}
```

## Best Practices

### TypeScript-Specific

#### Type Safety
- **FORBIDDEN**: Using `any` type (use `unknown` if type is truly unknown)
- Use type guards to narrow `unknown` types
- Prefer `interface` over `type` for object shapes (extensible)
- Use `type` for unions, intersections, and mapped types
- Enable strict mode in tsconfig.json
- Use discriminated unions for complex state

**Good Example**:
```typescript
// Use unknown + type guard instead of any
function processData(data: unknown): string {
  if (typeof data === 'string') {
    return data.toUpperCase();
  }
  throw new Error('Data must be a string');
}

// Discriminated union for state
type LoadingState = { status: 'loading' };
type SuccessState = { status: 'success'; data: User };
type ErrorState = { status: 'error'; error: Error };
type State = LoadingState | SuccessState | ErrorState;
```

**Bad Example**:
```typescript
// FORBIDDEN: Using any
function processData(data: any): string {
  return data.toUpperCase(); // No type safety!
}
```

#### Null Safety
- Use optional chaining (`?.`) and nullish coalescing (`??`)
- Avoid `null` when possible, prefer `undefined`
- Use strict null checks

```typescript
// Good
const userName = user?.profile?.name ?? 'Anonymous';

// Bad - doesn't handle undefined
const userName = user.profile.name || 'Anonymous';
```

### Idiomatic JavaScript/TypeScript

#### Prefer Modern Syntax
- Use `const` and `let`, never `var`
- Use arrow functions for callbacks and short functions
- Use template literals over string concatenation
- Use destructuring for objects and arrays
- Use spread operator over Object.assign
- Use async/await over raw promises

```typescript
// Good
const greet = (name: string) => `Hello, ${name}!`;
const { firstName, lastName } = user;
const updatedUser = { ...user, email: 'new@email.com' };

// Bad
var greet = function(name) {
  return 'Hello, ' + name + '!';
};
var firstName = user.firstName;
var lastName = user.lastName;
var updatedUser = Object.assign({}, user, { email: 'new@email.com' });
```

#### Immutability
- Prefer immutable data transformations
- Use array methods: map, filter, reduce (not for loops when possible)
- Don't mutate function arguments
- Use readonly modifier for TypeScript types when appropriate

```typescript
// Good
const doubled = numbers.map(n => n * 2);
const evens = numbers.filter(n => n % 2 === 0);

// Bad - mutation
numbers.forEach((n, i) => {
  numbers[i] = n * 2;
});
```

### Error Handling
- Use custom Error classes for different error types
- Always handle promise rejections
- Use try/catch for async/await
- Provide meaningful error messages
- Log errors with context

```typescript
// Custom error class
class ApiError extends Error {
  constructor(
    message: string,
    public statusCode: number,
    public response?: unknown
  ) {
    super(message);
    this.name = 'ApiError';
  }
}

// Usage with proper error handling
async function fetchData(url: string): Promise<Data> {
  try {
    const response = await fetch(url);
    if (!response.ok) {
      throw new ApiError(
        `Failed to fetch data: ${response.statusText}`,
        response.status
      );
    }
    return await response.json();
  } catch (error) {
    if (error instanceof ApiError) {
      console.error('API Error:', error.message, error.statusCode);
    } else {
      console.error('Unexpected error:', error);
    }
    throw error;
  }
}
```

### Testing
- Write tests for all public functions and components
- Use descriptive test names: `it('should return user data when API call succeeds')`
- Follow AAA pattern: Arrange, Act, Assert
- Mock external dependencies
- Aim for 80%+ code coverage for critical paths
- Test error cases and edge cases

```typescript
describe('UserService', () => {
  describe('fetchUser', () => {
    it('should return user data when API call succeeds', async () => {
      // Arrange
      const mockUser = { id: '1', name: 'John' };
      mockApiClient.get.mockResolvedValue(mockUser);

      // Act
      const result = await userService.fetchUser('1');

      // Assert
      expect(result).toEqual(mockUser);
      expect(mockApiClient.get).toHaveBeenCalledWith('/users/1');
    });

    it('should throw ApiError when API call fails', async () => {
      // Arrange
      mockApiClient.get.mockRejectedValue(new Error('Network error'));

      // Act & Assert
      await expect(userService.fetchUser('1')).rejects.toThrow(ApiError);
    });
  });
});
```

### Performance
- Avoid unnecessary re-renders in React (use React.memo, useMemo, useCallback)
- Debounce/throttle expensive operations
- Lazy load large dependencies
- Use code splitting for large applications
- Profile before optimizing (don't premature optimize)

### Security
- Sanitize user input
- Use Content Security Policy (CSP) headers
- Avoid eval() and Function constructor
- Validate data at boundaries (API responses, user input)
- Use environment variables for secrets (never commit secrets)
- Implement rate limiting for API calls

## Valid Code Requirements

Code is considered valid when:
- [x] Passes Prettier formatting
- [x] Passes ESLint with zero warnings
- [x] TypeScript compiler has zero errors
- [x] All tests pass
- [x] JSDoc comments present for all exports
- [x] Follows all naming conventions
- [x] No `any` types used
- [x] Proper error handling implemented
- [x] Test coverage meets requirements

### Pre-commit Checklist
```bash
# Format code
npm run format  # or: prettier --write .

# Lint code
npm run lint    # or: eslint . --ext .ts,.tsx

# Type check
npm run typecheck  # or: tsc --noEmit

# Run tests
npm run test    # or: jest or vitest
```

## Common Pitfalls

### Pitfall 1: Using `any` Type
**Problem**: Using `any` defeats TypeScript's type safety and can lead to runtime errors.
**Solution**: Use `unknown` for truly unknown types, then narrow with type guards. Use proper types everywhere.

### Pitfall 2: Not Handling Promise Rejections
**Problem**: Unhandled promise rejections can crash Node.js applications or cause silent failures.
**Solution**: Always use try/catch with async/await or .catch() with promises.

### Pitfall 3: Mutating Props or State
**Problem**: Direct mutation can cause unexpected behavior, especially in React.
**Solution**: Always create new objects/arrays. Use spread operator or immutable update patterns.

### Pitfall 4: Ignoring TypeScript Errors
**Problem**: Using `@ts-ignore` or `@ts-expect-error` to suppress valid errors.
**Solution**: Fix the actual type issue. Only use suppressions for third-party library issues.

### Pitfall 5: Not Cleaning Up Side Effects
**Problem**: Forgotten event listeners, timers, or subscriptions cause memory leaks.
**Solution**: Always clean up in React useEffect return function or component unmount.

### Pitfall 6: Blocking the Event Loop
**Problem**: Synchronous heavy computation blocks JavaScript's single thread.
**Solution**: Use Web Workers, break work into chunks, or use async operations.

### Pitfall 7: == Instead of ===
**Problem**: `==` does type coercion which can lead to unexpected results.
**Solution**: Always use `===` and `!==` for comparisons.

## Examples

### Good Example: Type-Safe API Client
```typescript
interface ApiResponse<T> {
  data: T;
  status: number;
  message: string;
}

class ApiClient {
  constructor(private baseUrl: string) {}

  async get<T>(endpoint: string): Promise<T> {
    try {
      const response = await fetch(`${this.baseUrl}${endpoint}`);

      if (!response.ok) {
        throw new ApiError(
          `GET ${endpoint} failed`,
          response.status
        );
      }

      const data: ApiResponse<T> = await response.json();
      return data.data;
    } catch (error) {
      if (error instanceof ApiError) {
        throw error;
      }
      throw new ApiError('Network request failed', 0);
    }
  }
}

// Usage with full type safety
const client = new ApiClient('https://api.example.com');
const user: User = await client.get<User>('/users/1');
```

**Why This is Good**:
- Fully typed with generics
- Proper error handling
- Clear error types
- No `any` types
- Follows async/await pattern

### Bad Example: Unsafe API Client
```typescript
// BAD - Don't do this!
class ApiClient {
  async get(endpoint: any): Promise<any> {  // Using any
    const response = await fetch(endpoint);  // No error handling
    return await response.json();  // No validation
  }
}

// Usage has no type safety
const user = await client.get('/users/1');  // user is any!
console.log(user.nama);  // Typo won't be caught (should be 'name')
```

**Why This is Bad**:
- Uses `any` type
- No error handling
- No HTTP status checking
- No type safety for consumers
- Typos won't be caught

**How to Fix**: Use the good example above with proper types and error handling.

### Good Example: React Component with TypeScript
```typescript
interface UserCardProps {
  user: User;
  onEdit: (userId: string) => void;
}

export const UserCard: React.FC<UserCardProps> = ({ user, onEdit }) => {
  const handleEdit = () => {
    onEdit(user.id);
  };

  return (
    <div className="user-card">
      <h3>{user.name}</h3>
      <p>{user.email}</p>
      <button onClick={handleEdit}>Edit</button>
    </div>
  );
};
```

**Why This is Good**:
- Proper TypeScript props interface
- Exported for reusability
- Clear prop types
- Simple, focused component

### Bad Example: React Component Without Types
```typescript
// BAD - Don't do this!
export const UserCard = ({ user, onEdit }) => {  // No types!
  return (
    <div>
      <h3>{user.name}</h3>
      <p>{user.email}</p>
      <button onClick={() => onEdit(user)}>Edit</button>  // Wrong signature!
    </div>
  );
};
```

**Why This is Bad**:
- No prop types
- No type safety
- Callback signature not clear
- Easy to make mistakes

## Learning Log

### 2026-01-11: Initial JavaScript/TypeScript Standards
**Issue**: Creating initial standards document.
**Learning**: Established baseline standards for JavaScript and TypeScript development in this project.
**Corrective Action**: None (initial creation).
**New Standard**: All JavaScript/TypeScript code must follow these standards starting from this date.

---
*Created: 2026-01-11*
*Last Updated: 2026-01-11*
