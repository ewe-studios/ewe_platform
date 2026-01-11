# Python Coding Standards

## Overview
- **Language**: Python 3.10+ (prefer 3.11 or 3.12)
- **Use Cases**: Data processing, scripting, automation, backend services, machine learning
- **Official Docs**: https://docs.python.org/3/

## Setup and Tools

### Required Tools
- **Python**: 3.10+ installed via pyenv or system package manager
- **pip**: Package installer (comes with Python)
- **black**: Opinionated code formatter
- **ruff**: Fast Python linter (replaces flake8, isort, etc.)
- **mypy**: Static type checker
- **pytest**: Testing framework
- **poetry** or **uv**: Modern dependency management (recommended)

### Installation
```bash
# Using pip
pip install black ruff mypy pytest

# Or using poetry
poetry add --group dev black ruff mypy pytest

# Or using uv (fastest)
uv pip install black ruff mypy pytest
```

### Configuration Files
- **pyproject.toml**: Project metadata, dependencies, tool configs
- **requirements.txt** or **poetry.lock**: Dependency lock file
- **.python-version**: Python version specification
- **pytest.ini** or pyproject.toml: Pytest configuration
- **mypy.ini** or pyproject.toml: Mypy configuration

### Recommended pyproject.toml Configuration
```toml
[tool.black]
line-length = 100
target-version = ['py310', 'py311', 'py312']

[tool.ruff]
line-length = 100
target-version = "py310"
select = [
    "E",   # pycodestyle errors
    "W",   # pycodestyle warnings
    "F",   # pyflakes
    "I",   # isort
    "N",   # pep8-naming
    "UP",  # pyupgrade
    "B",   # flake8-bugbear
    "C4",  # flake8-comprehensions
    "SIM", # flake8-simplify
]

[tool.mypy]
python_version = "3.10"
strict = true
warn_return_any = true
warn_unused_configs = true
disallow_untyped_defs = true
disallow_any_unimported = true

[tool.pytest.ini_options]
testpaths = ["tests"]
python_files = ["test_*.py"]
python_functions = ["test_*"]
addopts = "-v --tb=short --strict-markers"
```

## Coding Standards

### Naming Conventions (PEP 8)
- **Variables and Functions**: snake_case
  - `user_name = "John"`
  - `def calculate_total():`
- **Classes**: PascalCase
  - `class UserAccount:`
- **Constants**: UPPER_SNAKE_CASE
  - `MAX_RETRIES = 3`
- **Private attributes/methods**: _leading_underscore
  - `def _internal_method():`
  - `self._private_field = value`
- **"Really" private (name mangling)**: __double_underscore
  - `self.__very_private = value` (rarely needed)
- **Modules**: short, lowercase, no underscores if possible
  - `import users` not `import user_management_system`
- **Packages**: short, lowercase, no underscores
  - `myapp.models` not `my_app.data_models`
- **Files**: snake_case
  - `user_service.py`, `api_client.py`
- **Test files**: `test_*.py` or `*_test.py`
  - `test_user_service.py`

### Code Organization
- One class per file for large classes
- Group related functions in modules
- Use `__init__.py` to expose public API
- Separate concerns: models, services, utilities
- Keep files under 500 lines (guideline, not hard limit)

**Package Structure Example**:
```
src/
├── __init__.py
├── models/
│   ├── __init__.py
│   ├── user.py
│   └── post.py
├── services/
│   ├── __init__.py
│   ├── user_service.py
│   └── auth_service.py
└── utils/
    ├── __init__.py
    └── validators.py

tests/
├── __init__.py
├── test_user_service.py
└── test_validators.py
```

### Comments and Documentation
- Use docstrings for all modules, classes, and functions (Google or NumPy style)
- Type hints are mandatory for function signatures
- Inline comments for complex logic only
- Follow PEP 257 for docstring conventions

**Docstring Example (Google Style)**:
```python
def fetch_user(user_id: int, include_posts: bool = False) -> User | None:
    """Fetch a user from the database by ID.

    Args:
        user_id: The unique identifier for the user.
        include_posts: Whether to eagerly load user's posts. Defaults to False.

    Returns:
        The User object if found, None otherwise.

    Raises:
        DatabaseError: If database connection fails.
        ValidationError: If user_id is invalid.

    Example:
        >>> user = fetch_user(123)
        >>> print(user.name)
        'John Doe'
    """
    ...
```

## Best Practices

### Type Hints (Mandatory)
- **MANDATORY**: Type hints for all function signatures
- Use `from __future__ import annotations` for forward references
- Use `typing` module types: `List`, `Dict`, `Optional`, `Union`
- Python 3.10+ union syntax: `str | None` instead of `Optional[str]`
- Use `typing.Protocol` for structural subtyping
- Run `mypy` to check type correctness

```python
from __future__ import annotations

from typing import Protocol

class Validator(Protocol):
    """Protocol for validators (structural typing)."""
    def validate(self, value: str) -> bool: ...

def process_users(
    users: list[User],
    validator: Validator | None = None
) -> dict[str, list[User]]:
    """Process users and group by status.

    Args:
        users: List of users to process.
        validator: Optional validator to apply.

    Returns:
        Dictionary mapping status to list of users.
    """
    result: dict[str, list[User]] = {}
    for user in users:
        if validator and not validator.validate(user.email):
            continue
        result.setdefault(user.status, []).append(user)
    return result
```

### Pythonic Code Idioms

#### Use Context Managers
```python
# Good - context manager handles cleanup
with open('file.txt') as f:
    content = f.read()

# Good - custom context manager
from contextlib import contextmanager

@contextmanager
def database_transaction(db):
    """Context manager for database transactions."""
    db.begin()
    try:
        yield db
        db.commit()
    except Exception:
        db.rollback()
        raise
```

#### List Comprehensions and Generator Expressions
```python
# Good - list comprehension
doubled = [x * 2 for x in numbers]

# Good - generator expression (memory efficient)
sum_doubled = sum(x * 2 for x in numbers)

# Good - with condition
evens = [x for x in numbers if x % 2 == 0]

# Bad - unnecessary loop
doubled = []
for x in numbers:
    doubled.append(x * 2)
```

#### Avoid Mutable Default Arguments
```python
# Bad - mutable default argument
def add_item(item, items=[]):  # WRONG!
    items.append(item)
    return items

# Good - use None and create new list
def add_item(item: str, items: list[str] | None = None) -> list[str]:
    if items is None:
        items = []
    items.append(item)
    return items
```

#### Use Dataclasses for Data Containers
```python
from dataclasses import dataclass, field

@dataclass
class User:
    """User model with automatic __init__, __repr__, etc."""
    id: int
    name: str
    email: str
    roles: list[str] = field(default_factory=list)

    def is_admin(self) -> bool:
        """Check if user has admin role."""
        return 'admin' in self.roles
```

#### Use Enums for Constants
```python
from enum import Enum, auto

class UserStatus(Enum):
    """User status enumeration."""
    ACTIVE = auto()
    INACTIVE = auto()
    BANNED = auto()

# Usage
user.status = UserStatus.ACTIVE
if user.status == UserStatus.BANNED:
    raise PermissionError("User is banned")
```

### Error Handling
- Use specific exception types
- Create custom exceptions for domain errors
- Use `raise ... from ...` to preserve exception context
- Don't catch generic `Exception` unless re-raising
- Use `try/finally` or context managers for cleanup

```python
class UserError(Exception):
    """Base exception for user-related errors."""

class UserNotFoundError(UserError):
    """Raised when user is not found."""
    def __init__(self, user_id: int):
        self.user_id = user_id
        super().__init__(f"User {user_id} not found")

def fetch_user(user_id: int) -> User:
    """Fetch user by ID.

    Raises:
        UserNotFoundError: If user doesn't exist.
        DatabaseError: On database connection issues.
    """
    try:
        user = db.query(User).filter_by(id=user_id).one()
    except NoResultFound as e:
        raise UserNotFoundError(user_id) from e
    except SQLAlchemyError as e:
        raise DatabaseError("Failed to fetch user") from e

    return user
```

### Testing with pytest
- Write tests for all public functions and classes
- Use descriptive test names: `test_<what>_<condition>_<expected>`
- Use fixtures for setup/teardown
- Use parametrize for multiple test cases
- Aim for 80%+ code coverage

```python
import pytest
from myapp.services import UserService
from myapp.models import User

@pytest.fixture
def user_service():
    """Create a UserService instance for testing."""
    return UserService(db=MockDatabase())

@pytest.fixture
def sample_user():
    """Create a sample user for testing."""
    return User(id=1, name="John", email="john@example.com")

class TestUserService:
    """Tests for UserService."""

    def test_create_user_success(self, user_service):
        """Test creating a user succeeds with valid data."""
        user = user_service.create_user("John", "john@example.com")
        assert user.name == "John"
        assert user.email == "john@example.com"

    def test_create_user_invalid_email(self, user_service):
        """Test creating a user fails with invalid email."""
        with pytest.raises(ValidationError):
            user_service.create_user("John", "invalid-email")

    @pytest.mark.parametrize("email", [
        "test@example.com",
        "user.name@domain.co.uk",
        "user+tag@example.com",
    ])
    def test_valid_emails(self, user_service, email):
        """Test that various valid email formats are accepted."""
        user = user_service.create_user("Test", email)
        assert user.email == email
```

### Async/Await
- Use `async`/`await` for I/O-bound operations
- Use `asyncio` for async task management
- Libraries: `httpx` (HTTP), `asyncpg` (Postgres), `motor` (MongoDB)
- Don't mix sync and async without proper handling

```python
import asyncio
import httpx

async def fetch_user_data(user_id: int) -> dict:
    """Fetch user data from remote API.

    Args:
        user_id: ID of user to fetch.

    Returns:
        Dictionary containing user data.

    Raises:
        httpx.HTTPError: On HTTP request failure.
    """
    async with httpx.AsyncClient() as client:
        response = await client.get(f"https://api.example.com/users/{user_id}")
        response.raise_for_status()
        return response.json()

async def process_multiple_users(user_ids: list[int]) -> list[dict]:
    """Fetch data for multiple users concurrently.

    Args:
        user_ids: List of user IDs to fetch.

    Returns:
        List of user data dictionaries.
    """
    tasks = [fetch_user_data(uid) for uid in user_ids]
    return await asyncio.gather(*tasks)
```

### Performance
- Profile before optimizing (`cProfile`, `line_profiler`)
- Use generators for large datasets
- Use `slots` in classes with many instances
- Avoid global variables
- Use `functools.lru_cache` for expensive pure functions

```python
from functools import lru_cache

@lru_cache(maxsize=128)
def expensive_computation(n: int) -> int:
    """Cached expensive computation."""
    # Expensive operation here
    return result

# Using __slots__ for memory efficiency
class Point:
    """Memory-efficient point class."""
    __slots__ = ('x', 'y')

    def __init__(self, x: float, y: float):
        self.x = x
        self.y = y
```

### Security
- Never use `eval()` or `exec()` on user input
- Validate and sanitize all input
- Use parameterized queries (not string formatting)
- Don't store secrets in code (use environment variables)
- Use `secrets` module for cryptographic random numbers

```python
import os
import secrets
from typing import Any

# Good - environment variables for secrets
DATABASE_URL = os.environ["DATABASE_URL"]
API_KEY = os.environ.get("API_KEY", "")

# Good - parameterized query
def fetch_user_by_email(email: str) -> User | None:
    """Fetch user by email using safe parameterized query."""
    query = "SELECT * FROM users WHERE email = ?"
    return db.execute(query, (email,)).fetchone()

# Bad - SQL injection vulnerability
def fetch_user_by_email_unsafe(email: str) -> User | None:
    """DON'T DO THIS - SQL injection vulnerable!"""
    query = f"SELECT * FROM users WHERE email = '{email}'"  # DANGEROUS!
    return db.execute(query).fetchone()

# Good - secure random token
def generate_token() -> str:
    """Generate a secure random token."""
    return secrets.token_urlsafe(32)
```

## Valid Code Requirements

Code is considered valid when:
- [x] Passes `black --check .` (formatted correctly)
- [x] Passes `ruff check .` (no linter errors)
- [x] Passes `mypy .` (type checks pass)
- [x] Passes `pytest` (all tests pass)
- [x] Has type hints for all function signatures
- [x] Has docstrings for all public functions/classes
- [x] Follows all naming conventions
- [x] Test coverage meets requirements (80%+)

### Pre-commit Checklist
```bash
# Format code
black .

# Lint code
ruff check .

# Type check
mypy .

# Run tests
pytest

# Optional: check coverage
pytest --cov=src --cov-report=term-missing
```

## Common Pitfalls

### Pitfall 1: Mutable Default Arguments
**Problem**: Mutable default arguments are shared across function calls.
**Solution**: Use `None` and create new mutable object inside function.

```python
# Bad
def add_item(item, items=[]):  # Shared across calls!
    items.append(item)
    return items

# Good
def add_item(item: str, items: list[str] | None = None) -> list[str]:
    if items is None:
        items = []
    items.append(item)
    return items
```

### Pitfall 2: Not Using Type Hints
**Problem**: No static type checking, harder to maintain.
**Solution**: Always add type hints to function signatures.

### Pitfall 3: Catching Too Broad Exceptions
**Problem**: `except Exception:` catches everything, hiding real errors.
**Solution**: Catch specific exception types.

```python
# Bad
try:
    result = risky_operation()
except Exception:  # Too broad!
    result = None

# Good
try:
    result = risky_operation()
except (NetworkError, TimeoutError) as e:
    logger.error(f"Operation failed: {e}")
    result = None
```

### Pitfall 4: Not Closing Resources
**Problem**: File handles, network connections left open.
**Solution**: Always use context managers.

```python
# Bad
f = open('file.txt')
content = f.read()
f.close()  # Might not be called if exception occurs!

# Good
with open('file.txt') as f:
    content = f.read()  # Automatically closed
```

### Pitfall 5: Using `import *`
**Problem**: Pollutes namespace, unclear where names come from.
**Solution**: Import specific names or use qualified imports.

```python
# Bad
from module import *  # What did we import?

# Good
from module import specific_function, SpecificClass

# Good - qualified import
import module
module.specific_function()
```

### Pitfall 6: Ignoring PEP 8
**Problem**: Inconsistent code style.
**Solution**: Use `black` formatter and `ruff` linter.

## Examples

### Good Example: Type-Safe Service Class
```python
from __future__ import annotations

from dataclasses import dataclass
from typing import Protocol

class UserRepository(Protocol):
    """Protocol for user data access."""
    def find_by_id(self, user_id: int) -> User | None: ...
    def save(self, user: User) -> None: ...

@dataclass
class User:
    """User model."""
    id: int
    name: str
    email: str

class UserService:
    """Service for user management operations."""

    def __init__(self, repository: UserRepository):
        """Initialize service with repository.

        Args:
            repository: User data repository.
        """
        self._repository = repository

    def get_user(self, user_id: int) -> User:
        """Get user by ID.

        Args:
            user_id: ID of user to fetch.

        Returns:
            The user object.

        Raises:
            UserNotFoundError: If user doesn't exist.
        """
        user = self._repository.find_by_id(user_id)
        if user is None:
            raise UserNotFoundError(user_id)
        return user

    def update_email(self, user_id: int, new_email: str) -> User:
        """Update user's email address.

        Args:
            user_id: ID of user to update.
            new_email: New email address.

        Returns:
            Updated user object.

        Raises:
            UserNotFoundError: If user doesn't exist.
            ValidationError: If email format is invalid.
        """
        user = self.get_user(user_id)
        if not self._is_valid_email(new_email):
            raise ValidationError(f"Invalid email: {new_email}")

        user.email = new_email
        self._repository.save(user)
        return user

    def _is_valid_email(self, email: str) -> bool:
        """Validate email format.

        Args:
            email: Email address to validate.

        Returns:
            True if valid, False otherwise.
        """
        return '@' in email and '.' in email.split('@')[1]
```

**Why This is Good**:
- Full type hints
- Protocol for dependency injection
- Comprehensive docstrings
- Custom exceptions
- Separation of concerns
- Private method with underscore prefix

### Bad Example: Untyped, Unsafe Code
```python
# BAD - Don't do this!
class UserService:
    def __init__(self, db):  # No type hints!
        self.db = db  # Public attribute

    def get_user(self, id):  # No type hints, no docstring
        user = self.db.execute(f"SELECT * FROM users WHERE id = {id}").fetchone()  # SQL injection!
        if user == None:  # Should use 'is None'
            raise Exception("Not found")  # Generic exception
        return user

    def update_email(self, id, email):
        user = self.get_user(id)
        user['email'] = email  # Mutating dictionary
        self.db.execute(f"UPDATE users SET email = '{email}' WHERE id = {id}")  # More SQL injection!
```

**Why This is Bad**:
- No type hints
- No docstrings
- SQL injection vulnerabilities
- Generic exceptions
- Public db attribute
- Using `==` instead of `is` for None check
- Inconsistent style

**How to Fix**: Use the good example above with proper typing, documentation, and security.

## Learning Log

### 2026-01-11: Initial Python Standards
**Issue**: Creating initial standards document.
**Learning**: Established baseline standards for Python development in this project.
**Corrective Action**: None (initial creation).
**New Standard**: All Python code must follow these standards starting from this date.

---
*Created: 2026-01-11*
*Last Updated: 2026-01-11*
