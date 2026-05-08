# TW Stock Analysis System

A backend system for Taiwan stock data analysis, strategy backtesting,
and data synchronization.

This project is designed with a focus on clean architecture,
scalability, and maintainability, making it suitable for financial data
processing and quantitative analysis systems.

------------------------------------------------------------------------

## Features

-   Taiwan Stock Data Analysis
-   Data Synchronization (FinMind API)
-   Strategy Backtesting Engine (in progress)
-   Layered Architecture (API / Service / Domain / Infra)
-   Frontend interface (development mode supported)

------------------------------------------------------------------------

## Tech Stack

### Backend

-   **Language:** Rust
-   **Async Runtime:** Tokio
-   **Database / ORM:** PostgreSQL, SQLx
-   **Caching / In-Memory:** Redis
-   **Serialization:** Serde

### Frontend

-   Node.js, Next.js, React

### Database

-   PostgreSQL
-   Redis

### Architecture

Layered Architecture (inspired by Clean Architecture / DDD)

-   API Layer
-   Service Layer
-   Domain Layer
-   Infrastructure Layer

### External Services

-   FinMind API (Taiwan stock data)

------------------------------------------------------------------------

## System Architecture

    Client
      ↓
    API Layer (Request / Response)
      ↓
    Service Layer (Use Case / Orchestration)
      ↓         ↘
    Domain Layer   Infrastructure Layer
    (Core Logic)   (PostgreSQL / Redis / FinMind API)

> Domain layer is designed to be independent from infrastructure.\
> Currently, the service layer directly interacts with infrastructure.\
> Introducing repository interfaces is a future improvement.

------------------------------------------------------------------------

## Project Structure

    src/
    ├── api/
    │   ├── request/
    │   ├── response/
    │   └── handler/
    │
    ├── service/
    │   ├── command/
    │   └── usecase/
    │
    ├── domain/
    │   ├── model/
    │   └── logic/
    │
    ├── infra/
    │   ├── finmind/
    │   └── db/
    │
    └── main.rs

------------------------------------------------------------------------

## Quick Start

### Prerequisites

- Rust (cargo)
- PostgreSQL
- Redis
- Node.js

## Backend Usage

### Run the server

``` bash
# Clone the repository
git clone https://github.com/ejzilch/twstock_analysis.git
cd twstock_analysis

# Setup environment variables
cp .env.example .env
# (Edit .env with your PostgreSQL and Redis credentials)

# Run database migrations
cargo sqlx database setup

# Start the Rust server
cargo run
```

------------------------------------------------------------------------

## Frontend Usage

### Install dependencies

``` bash
cd frontend
npm install
npm run dev
```

### Run development server

``` bash
npm run dev
```

### Open in browser

    http://localhost:3000

------------------------------------------------------------------------

## Design Highlights

### Separation of Concerns

-   Request → API layer input validation
-   Command → Service layer use case
-   Domain → Core business logic

### Benefits

-   Decoupled architecture
-   Easier refactoring
-   Better testability

------------------------------------------------------------------------

## Current Limitations

-   Backtesting engine not fully implemented
-   No unified error handling
-   No async job handling
-   Struct duplication across layers

------------------------------------------------------------------------

## Future Improvements

### Backtesting Engine

-   [ ] Strategy execution engine
-   [ ] Plugin-based strategies
-   [ ] Parallel processing

### Data Sync

-   [ ] Retry mechanism
-   [ ] Rate limit handling
-   [ ] Incremental sync

### Architecture

-   [ ] Repository pattern
-   [ ] Dependency Injection
-   [ ] Modularization

### Infrastructure

-   [ ] Message Queue

### DevOps

-   [ ] Docker
-   [ ] CI/CD

------------------------------------------------------------------------

## Use Cases

-   Algorithmic trading systems
-   Financial data platforms
-   Quantitative research tools

------------------------------------------------------------------------

## Author

Zilch Feng