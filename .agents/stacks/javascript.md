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

## Code Verification Workflow

### Overview

**MANDATORY**: Every code change in JavaScript/TypeScript MUST be verified by a dedicated JavaScript Verification Agent before being committed. This is a **HARD REQUIREMENT** with **ZERO TOLERANCE** for violations.

### Verification Agent Responsibility

There can only be **ONE JavaScript Verification Agent** active at any time for a given set of changes. The Main Agent is responsible for:

1. **Delegating** to the JavaScript Verification Agent after implementation is complete
2. **Waiting** for verification results before proceeding
3. **Not committing** any JavaScript/TypeScript code until verification passes
4. **Reporting** verification results to the user

### When Verification Must Run

Verification MUST run:
- ✅ After ANY code changes to `.js`, `.ts`, `.jsx`, `.tsx` files
- ✅ After changes to `package.json`, `package-lock.json`, or `tsconfig.json`
- ✅ After adding new dependencies
- ✅ After updating dependencies
- ✅ Before ANY commit containing JavaScript/TypeScript code
- ✅ After merging or rebasing branches

### Verification Agent Workflow

#### Step 1: Agent Delegation

**Main Agent** responsibilities:
```
1. Implementation agent completes JavaScript/TypeScript code changes
2. Implementation agent reports completion to Main Agent
3. Main Agent spawns ONE JavaScript Verification Agent
4. Main Agent provides verification agent with:
   - List of changed files
   - Description of changes made
   - Specification reference (if applicable)
5. Main Agent WAITS for verification results
```

**Verification Agent** receives:
- Context about what was changed
- Why it was changed
- Expected behavior
- Files modified

#### Step 2: Verification Agent Execution

The **JavaScript Verification Agent** MUST execute ALL of the following checks in order:

##### 1. Format Verification
```bash
npx prettier --check .
# or
npm run format:check
```
- **MUST PASS**: Code must be properly formatted
- **On Failure**: Run `prettier --write .` and report formatting issues
- **Zero Tolerance**: No unformatted code allowed

##### 2. TypeScript Type Check
```bash
npx tsc --noEmit
# or
npm run typecheck
```
- **MUST PASS**: Zero TypeScript errors allowed
- **On Failure**: Report ALL type errors with file locations
- **Zero Tolerance**: Fix all type errors before proceeding

##### 3. ESLint Check
```bash
npx eslint . --ext .ts,.tsx,.js,.jsx --max-warnings 0
# or
npm run lint
```
- **MUST PASS**: Zero lint warnings or errors allowed
- **On Failure**: Report ALL lint issues with file locations
- **Zero Tolerance**: Fix all lint issues before proceeding

##### 4. Test Execution
```bash
npm run test
# or
npx jest --coverage
# or
npx vitest run
```
- **MUST PASS**: All tests must pass
- **On Failure**: Report which tests failed and why
- **Verify**: Check test coverage meets requirements (80%+)

##### 5. Build Check (if applicable)
```bash
npm run build
# or
npx tsc
# or
npx vite build
```
- **MUST PASS**: Build must succeed without errors
- **On Failure**: Report build errors
- **Verify**: Production build completes successfully

##### 6. Dependency Audit
```bash
npm audit --audit-level=moderate
```
- **MUST PASS**: No moderate or higher severity vulnerabilities
- **On Warning**: Report vulnerabilities with severity
- **Action**: Update dependencies or document accepted risks

#### Step 3: Standards Compliance Verification

The Verification Agent MUST also verify compliance with this stack file:

##### Code Quality Checks
- [ ] No `any` type usage
  ```bash
  rg ": any" --type ts --type tsx
  rg "as any" --type ts --type tsx
  ```
  - Report any violations with file and line number
  - Exception: Third-party library type issues (must be documented)

- [ ] Proper error handling
  - All `async` functions have proper try/catch or error handling
  - Promises have `.catch()` handlers or try/catch
  - No unhandled promise rejections

- [ ] JSDoc comments for exports
  ```bash
  # ESLint should enforce this with require-jsdoc rule
  npx eslint . --rule "require-jsdoc: error"
  ```

- [ ] Naming conventions followed
  - camelCase for functions, variables
  - PascalCase for classes, interfaces, types
  - UPPER_SNAKE_CASE for constants

- [ ] No TypeScript error suppressions
  ```bash
  rg "@ts-ignore" --type ts --type tsx
  rg "@ts-expect-error" --type ts --type tsx
  ```
  - Report any suppressions
  - Must be justified with comment explaining why

##### React-Specific Checks (if applicable)
- [ ] No direct state mutation
- [ ] useEffect cleanup functions present
- [ ] Dependency arrays are correct
- [ ] No missing dependencies warnings
- [ ] Proper key props in lists

#### Step 4: Verification Report

The Verification Agent MUST generate a comprehensive report:

##### Report Format
```markdown
# JavaScript/TypeScript Verification Report

## Summary
- **Status**: PASS ✅ / FAIL ❌
- **Files Changed**: [list of files]
- **Verification Time**: [timestamp]

## Check Results

### 1. Format Check
- **Status**: PASS ✅ / FAIL ❌
- **Details**: [any issues found]

### 2. TypeScript Type Check
- **Status**: PASS ✅ / FAIL ❌
- **Errors**: [count]
- **Details**: [error messages]

### 3. ESLint Check
- **Status**: PASS ✅ / FAIL ❌
- **Warnings**: [count]
- **Errors**: [count]
- **Details**: [lint issues]

### 4. Tests
- **Tests Run**: [count]
- **Tests Passed**: [count]
- **Tests Failed**: [count]
- **Coverage**: [percentage]
- **Details**: [failure details]

### 5. Build Check
- **Status**: PASS ✅ / FAIL ❌
- **Details**: [any errors]

### 6. Dependency Audit
- **Status**: PASS ✅ / FAIL ❌
- **Vulnerabilities**: [count by severity]
- **Details**: [vulnerability list]

### 7. Standards Compliance
- **No `any` Type**: PASS ✅ / FAIL ❌
- **Error Handling**: PASS ✅ / FAIL ❌
- **JSDoc Comments**: PASS ✅ / FAIL ❌
- **Naming Conventions**: PASS ✅ / FAIL ❌
- **No TS Suppressions**: PASS ✅ / FAIL ❌

## Overall Assessment

[Detailed explanation of verification results]

## Recommendations

[Any suggestions for improvement]

## Blockers

[Any issues that prevent commit]
```

#### Step 5: Main Agent Response

Based on Verification Agent report:

##### If Verification PASSES (✅)
```
Main Agent actions:
1. Receives PASS report from Verification Agent
2. Reviews report for any warnings or recommendations
3. Commits the changes following Rule 03 (Work Commit Rules)
4. Includes verification summary in commit message:
   "Verified by JavaScript Verification Agent: All checks passed"
5. Pushes to remote following Rule 05 (Git Auto-Approval)
6. Reports success to user
```

##### If Verification FAILS (❌)
```
Main Agent actions:
1. Receives FAIL report from Verification Agent
2. DOES NOT COMMIT any code
3. Reports failures to implementation agent or user
4. Lists all issues that must be fixed:
   - Formatting issues
   - Type errors
   - Lint warnings
   - Test failures
   - Build errors
   - Standards violations
5. Implementation agent fixes issues
6. Repeats verification process
7. ONLY proceeds after PASS
```

### Verification Agent Requirements

The Verification Agent MUST:
- ✅ Be spawned by Main Agent ONLY
- ✅ Run ALL checks in order
- ✅ Generate comprehensive report
- ✅ Report results to Main Agent
- ✅ NOT commit any code (Main Agent's responsibility)
- ✅ NOT proceed with partial passes (all checks must pass)

The Verification Agent MUST NOT:
- ❌ Skip any verification checks
- ❌ Ignore failures ("we'll fix it later")
- ❌ Commit code directly
- ❌ Proceed when checks fail
- ❌ Run concurrently (only one per language stack)

### Example Workflow

#### Good Example ✅
```
1. User: "Add user profile component in React"
2. Main Agent: Creates specification
3. Main Agent: Spawns JavaScript Implementation Agent
4. Implementation Agent: Writes React component with TypeScript
5. Implementation Agent: Reports completion to Main Agent
6. Main Agent: Spawns JavaScript Verification Agent
7. Verification Agent: Runs all checks
8. Verification Agent: All checks PASS ✅
9. Verification Agent: Generates report
10. Verification Agent: Returns report to Main Agent
11. Main Agent: Reviews report
12. Main Agent: Commits code with verification note
13. Main Agent: Reports success to user
```

#### Bad Example ❌
```
1. User: "Add user profile component in React"
2. Main Agent: Creates specification
3. Main Agent: Spawns JavaScript Implementation Agent
4. Implementation Agent: Writes React component
5. Implementation Agent: Commits code directly ❌ VIOLATION!
   (Should have reported to Main Agent first)
6. Code uses `any` type ❌ VIOLATION!
7. Tests are failing ❌ VIOLATION!
8. No verification was run ❌ CRITICAL VIOLATION!

Result: Code quality compromised, standards violated
```

### Integration with Other Rules

#### Works With Rule 03 (Work Commit Rules)
- Verification happens BEFORE commit
- Commit message includes verification status
- Only verified code is committed

#### Works With Rule 04 (Agent Orchestration)
- Main Agent orchestrates verification
- Implementation agents don't commit directly
- Verification agent is specialized for quality checks

#### Works With Rule 06 (Specifications and Requirements)
- Verification agent receives specification context
- Tests verify requirements are met
- Verification report confirms completion

#### Works With Rule 07 (Language Conventions)
- Verification enforces stack standards
- Checks compliance with this document
- Updates Learning Log when new patterns discovered

### Enforcement

#### Zero Tolerance Policy

**VIOLATIONS** are treated with **ZERO TOLERANCE**:

- ❌ **FORBIDDEN**: Committing JavaScript/TypeScript code without verification
- ❌ **FORBIDDEN**: Skipping verification checks
- ❌ **FORBIDDEN**: Ignoring verification failures
- ❌ **FORBIDDEN**: Running verification after commit
- ❌ **FORBIDDEN**: Multiple concurrent verification agents

#### Violation Consequences

Any agent that violates verification requirements will:
1. Have their changes **REVERTED**
2. Be required to run verification properly
3. Fix ALL issues before re-attempting
4. Document the violation in Learning Log
5. Report the violation to user

#### User Impact

Violations have serious consequences:
- ❌ **Runtime errors** in production from type issues
- ❌ **Failed builds** discovered too late
- ❌ **Security vulnerabilities** undetected
- ❌ **Code quality degradation** over time
- ❌ **Technical debt** accumulation
- ❌ **User trust** in agent reliability lost

**THE USER WILL BE UPSET** if code is committed without proper verification!

### Verification Commands Quick Reference

```bash
# Complete verification suite (run in order)

# 1. Format
npx prettier --check .

# 2. Type Check
npx tsc --noEmit

# 3. Lint
npx eslint . --ext .ts,.tsx,.js,.jsx --max-warnings 0

# 4. Test
npm test

# 5. Build
npm run build

# 6. Audit
npm audit --audit-level=moderate

# 7. Standards Check
rg ": any" --type ts --type tsx
rg "as any" --type ts --type tsx
rg "@ts-ignore" --type ts --type tsx
rg "@ts-expect-error" --type ts --type tsx

# All checks must PASS before commit
```

### Continuous Improvement

When verification catches issues:
1. **Document the issue** in Learning Log
2. **Explain why it was wrong**
3. **Show the correct approach**
4. **Update examples** if needed
5. **Commit Learning Log** update

This creates a self-improving system where standards evolve based on real issues encountered.

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
