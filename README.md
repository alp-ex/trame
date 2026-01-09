# Trame

A simple note-taking app. One user, one note.

## Stack

- **Backend**: Rust with raw hyper (no framework)
- **Database**: SQLite
- **Frontend**: Vanilla HTML/CSS/JS

---

## Quick Start

### Option 1: Local Development (without Docker)

```bash
cd server
cargo run
```

Visit `http://localhost:3000`

### Option 2: Docker Development (with hot reload)

```bash
docker compose --profile dev up
```

Visit `http://localhost:3000`

### Option 3: Docker Production

```bash
# 1. Configure your domain
cp .env.prod .env.prod.local
# Edit .env.prod.local and set ALLOWED_ORIGIN to your domain

# 2. Run
docker compose --profile prod up -d
```

---

## Environment Files

```
.env.example    # Reference for all variables (documentation only)
.env.dev        # Development settings (ready to use)
.env.prod       # Production template (copy and configure)
.env            # Local overrides (optional, gitignored)
```

### Which file do I use?

| Scenario | File to use | Command |
|----------|-------------|---------|
| Local dev (no Docker) | `.env` | `cargo run` |
| Docker development | `.env.dev` | `docker compose --profile dev up` |
| Docker production | `.env.prod` | `docker compose --profile prod up -d` |

### Environment Variables

| Variable | Default | Description |
|----------|---------|-------------|
| `PORT` | `3000` | Server port |
| `HOST` | `127.0.0.1` | Bind address (`0.0.0.0` in Docker) |
| `DATABASE_URL` | `trame.db` | SQLite database path |
| `ALLOWED_ORIGIN` | `*` | CORS origin (`*` for dev, your domain for prod) |
| `RUST_LOG` | `info` | Log level: `error`, `warn`, `info`, `debug`, `trace` |

### Setting up for Production

1. Copy the production template:
   ```bash
   cp .env.prod .env.prod.local
   ```

2. Edit `.env.prod.local` and change:
   ```bash
   ALLOWED_ORIGIN=https://yourdomain.com
   ```

3. Update `docker-compose.yml` to use your file:
   ```yaml
   env_file:
     - .env.prod.local
   ```

---

## Docker Commands

### Development

```bash
# Start with hot reload
docker compose --profile dev up

# Rebuild after dependency changes
docker compose --profile dev up --build

# View logs
docker compose --profile dev logs -f

# Stop
docker compose --profile dev down
```

### Production

```bash
# Start in background
docker compose --profile prod up -d

# View logs
docker compose --profile prod logs -f

# Stop
docker compose --profile prod down

# Rebuild and restart
docker compose --profile prod up -d --build
```

### Building Images Manually

```bash
# Build production image
docker build -t trame --target prod .

# Build development image
docker build -t trame:dev --target dev .

# Run production image
docker run -p 3000:10000 -v trame-data:/app/data trame
```

---

## API

| Method | Endpoint | Description |
|--------|----------|-------------|
| POST | `/api/signup` | Create account |
| POST | `/api/login` | Sign in |
| POST | `/api/logout` | Sign out |
| GET | `/api/note` | Get note |
| PUT | `/api/note` | Update note (auto-saves with 500ms debounce) |
| GET | `/api/health` | Health check |

---

## Tests

```bash
cd server
cargo test
```

---

## Deploy to Fly.io

```bash
# Install Fly CLI (if not already installed)
curl -L https://fly.io/install.sh | sh

# Login
fly auth login

# Launch (first time)
fly launch

# Deploy (subsequent times)
fly deploy
```

**Useful commands:**
```bash
# View logs
fly logs

# SSH into the machine
fly ssh console

# Inspect database
fly ssh console -C "sqlite3 /data/trame.db '.tables'"

# Check app status
fly status
```

Your app will be available at `https://trame.fly.dev`
