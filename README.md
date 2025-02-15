# dsgvo-downloader

`dsgvo-downloader` is a command-line tool written in Rust that scrapes and stores data about data breaches from the [dsgvo-portal.de](https://www.dsgvo-portal.de/sicherheitsvorfall-datenbank/) website. It fetches incident reports and their details, storing them in a PostgreSQL database.  The tool is designed to be respectful of the target website by implementing a configurable delay between requests.

## Features

*   **Fetches incident data:** Retrieves incident reports (Sicherheitsvorfälle) from dsgvo-portal.de.
*   **Fetches incident details:**  For each incident, fetches additional details from a separate details page.
*   **PostgreSQL database storage:** Persists fetched data in a PostgreSQL database, including raw JSON responses for historical analysis.
*   **Incremental updates:**  Only processes new incidents that are not already present in the database.
*   **Configurable request delay:**  Allows setting a delay between requests to avoid overloading the target website.
*   **Detailed logging:** Provides comprehensive logging at various levels (trace, debug, info, error) to help with troubleshooting and monitoring.
*   **Database schema verification:** Checks for the existence of required tables (`incidents` and `incident_history`) on startup.
*   **Stores raw responses**: Stores the raw response in a separate table.

## Prerequisites

*   **Rust:**  You need a working Rust installation (including Cargo).  The recommended way to install Rust is via [rustup](https://rustup.rs/).
*   **PostgreSQL:** A running PostgreSQL database instance is required.  You need to know the database URL (e.g., `postgres://user@host:port/database`).

## Database Setup with Docker (Tutorial)

This section provides a quick tutorial on setting up the required PostgreSQL database using Docker.  This is a convenient way to get a database running without needing to install PostgreSQL directly on your system.  We'll cover two approaches:

1.  **Using Docker Compose (Recommended):**  Simplest and most reliable for consistent setups.
2.  **Using a Single Docker Command:** More manual, but useful if you prefer not to use Docker Compose.

### Option 1: Docker Compose (Recommended)

Docker Compose allows you to define and manage multi-container applications. In this case, we'll use it to create a PostgreSQL container.

1.  **Create a `docker-compose.yml` file:** In the root directory of your `dsgvo-downloader` project (where your `src` folder is), create a file named `docker-compose.yml` with the following content:

    ```yaml
    services:
      postgres:
        image: postgres:latest  # You can specify a specific version if needed (e.g., postgres:14)
        restart: always      # Restart the container if it fails
        environment:
          POSTGRES_USER: postgres    # Change if you want a different username
          POSTGRES_AUTH_METHOD: trust
          POSTGRES_DB: dsgvo      # The database name (matches the default in --database-url)
        ports:
          - "5432:5432"          # Map port 5432 (Postgres) to the host
        volumes:
          - postgres_data:/var/lib/postgresql/data # Persist database data
          - ./src/schema.sql:/docker-entrypoint-initdb.d/schema.sql  # Initialize the database

    volumes:
      postgres_data:  # Define the named volume
    ```

2.  **Explanation of `docker-compose.yml`:**

    *   `services`: Defines the services (containers) that make up your application.
    *   `postgres`: The name of our PostgreSQL service.
    *   `image`: The Docker image to use.
    *   `restart`:  Restarts the container if it fails.
    *   `environment`: Sets environment variables for the PostgreSQL container.
        *   `POSTGRES_USER`:  The PostgreSQL username.
        *   `POSTGRES_AUTH_METHOD`:  Sets the postgres to trust every connection and to not require passwords
        *   `POSTGRES_DB`:  The name of the database to create.
    *   `ports`:  Maps the container's PostgreSQL port (5432) to the same port on your host machine.  This allows you to connect to the database from your host.
    *   `volumes`:
        *   `postgres_data:/var/lib/postgresql/data`:  Creates a *named volume* called `postgres_data`. This is crucial for *persisting* your database data.  Without this, your data would be lost when the container is removed. The data is stored outside the container's filesystem.
        *   `./src/schema.sql:/docker-entrypoint-initdb.d/schema.sql`: This is the most important part for initialization.  It copies your `schema.sql` file into a special directory within the container.  The official PostgreSQL image automatically executes SQL files in this directory *when the container is first created*. This is how your tables are set up.

3.  **Start the database:**

    ```bash
    docker-compose up -d
    ```

    *   `up`: Creates and starts the containers defined in `docker-compose.yml`.
    *   `-d`: Runs the containers in detached mode (in the background).

    The first time you run this, Docker will download the PostgreSQL image (if it's not already cached). The `schema.sql` file will be executed, creating your tables.

4. **Verify Database Connection**
    ```bash
     psql -h localhost -U postgres -d dsgvo -W
    ```
   Enter the password (`postgres` unless you changed it), and you should be connected to database.
   You can use `\dt` to list tables to confirm the tables exist.

5.  **Stop the database:**

    ```bash
    docker-compose down
    ```

    This stops and removes the containers.  However, because you used a named volume (`postgres_data`), your database data will be preserved.  The next time you run `docker-compose up -d`, the existing data will be used.

6. **Run your Rust application**
   You can now use the default database URL like so:
    ```bash
    ./target/release/dsgvo-downloader
    ```
### Option 2: Single Docker Command

This option achieves the same result as Docker Compose, but uses a single, more complex `docker` command.

1.  **Run the following command:**

    ```bash
    docker run -d --name dsgvo-db -e POSTGRES_USER=postgres -e POSTGRES_AUTH_METHOD=trust -e POSTGRES_DB=dsgvo -p 5432:5432 -v postgres_data:/var/lib/postgresql/data -v "$(pwd)"/src/schema.sql:/docker-entrypoint-initdb.d/schema.sql postgres:latest
    ```

    *   `-d`: Runs the container in detached mode.
    *   `--name dsgvo-db`:  Assigns a name to the container (for easier management).
    *   `-e`: Sets environment variables (same as in `docker-compose.yml`).
    *   `-p 5432:5432`: Maps the ports.
    *   `-v postgres_data:/var/lib/postgresql/data`: Creates the named volume for data persistence.
    *   `-v "$(pwd)"/src/schema.sql:/docker-entrypoint-initdb.d/schema.sql`:  Mounts the `schema.sql` file for database initialization.  `$(pwd)` gets the current working directory.
    *   `postgres:latest`: The Docker image.

2.  **Verify:**  Same as with Docker Compose.
    ```bash
     psql -h localhost -U postgres -d dsgvo
    ```

3.  **Stop the container:**

    ```bash
    docker stop dsgvo-db
    ```

4.  **Remove the container (optional):**

    ```bash
    docker rm dsgvo-db
    ```

    The data will *still* be preserved because of the named volume.  You can re-create the container later using the same `docker run` command.

5.  **Remove the volume (optional, BE CAREFUL):**  If you *really* want to delete the database data, you can remove the volume:

    ```bash
    docker volume rm postgres_data
    ```

    **WARNING:** This will permanently delete your database.  There is no undo.

## Installation

1.  **Clone the repository:**

    ```bash
    git clone git@github.com:Lucy-dot-dot/dsgvo-downloader-rs.git
    cd dsgvo-downloader
    ```

2.  **Build the project:**

    ```bash
    cargo build --release
    ```

    This will create an executable file in the `target/release` directory.

## Usage

```bash
dsgvo-downloader [OPTIONS]
```
### Command line options

*    **`-d, --delay <DELAY>` (default: 500):** Delay time in milliseconds between requests to `dsgvo-portal.de`.  The minimum value is 500ms. This is crucial to avoid overwhelming the server.
*    **`-u, --database-url <DATABASE_URL>` (default: `postgres://postgres@localhost:5432/dsgvo`):**  The PostgreSQL database connection URL. The tables must be preconfigured using `schema.sql`.  The format is a standard PostgreSQL connection string.
*   **`-h,--help`**: Prints help information

### Example

To run the tool with a delay of 1 second (1000 milliseconds) and connect to a database at a custom location:

```bash
./target/release/dsgvo-downloader --delay 1000 --database-url "postgres://user@db.example.com:5432/mydatabase"
```

## Database Schema

The tool uses two tables in your PostgreSQL database:

*   **`incidents`:** Stores detailed information about each incident.  This includes data from both the main incident list and the individual incident detail pages.

    | Column           | Type                       | Description                                                                                                 |
    | ---------------- | -------------------------- | ----------------------------------------------------------------------------------------------------------- |
    | `incident_id`    | `INTEGER` (Primary Key)    | Unique identifier for the incident, from dsgvo-portal.de.                                                       |
    | `org_publish_date` | `DATE`                    | Original publish date, as reported by the affected organization.                                          |
    | `modified_date`  | `TIMESTAMP WITH TIME ZONE` | Last modified date of the incident report.                                                                   |
    | `published`      | `INTEGER`                  |  (Unclear from the code what this field represents)                                                            |
    | `publish_date`   | `TIMESTAMP WITH TIME ZONE` | Publish date from the incident details.                                                                         |
    | `affected_obj`   | `TEXT`                    | Affected object, from the incident details.                                                                  |
    | `affected_type`  | `TEXT`                    | Type of affected object.                                                                              |
    | `country`        | `TEXT`                    | Country where the incident occurred.                                                                     |
    | `details_text`   | `TEXT`                    | Detailed description of the incident, in German.                                                            |
    | `tags`           | `TEXT`                    | Tags associated with the incident.                                                                           |
    | `href`           | `TEXT`                    |  URL to the incident report                                            |
    | `references`     | `JSONB`                   | References related to details, stored as JSON.                                                              |
    | `incident_text`  | `TEXT`                    | Text of the incident report.                                                                               |

*   **`incident_history`:**  Stores the raw JSON response from the initial incident list fetch (`cmd=getIncidents`). This is useful for historical analysis and debugging.

    | Column       | Type                       | Description                                                                            |
    | ------------ | -------------------------- | -------------------------------------------------------------------------------------- |
    | `id`         | `SERIAL` (Primary Key)    | Auto-incrementing primary key.                                                          |
    | `content`    | `JSONB`                   | The raw JSON content of the response.                                                  |
    | `created_at` | `TIMESTAMP WITH TIME ZONE` | Timestamp indicating when the response was stored (defaults to the current timestamp). |

## Logging

The tool uses the `env_logger` and `log` crates for logging.  By default, it logs at the `info` level. You can control the logging level using environment variables:

*   **General logging level:**  Set the `RUST_LOG` environment variable.  For example, to see debug messages:

    ```bash
    RUST_LOG=debug ./target/release/dsgvo-downloader ...
    ```

*   **Module-specific logging:** The tool specifically sets the logging level for the `dsgvo_downloader` module to `trace`.  This means you can see very detailed trace messages from the tool itself, even if you set a higher level globally.  To see only error messages for everything, besides the tool you can:

     ```bash
     RUST_LOG=error,dsgvo_downloader=trace ./target/release/dsgvo-downloader ...
     ```

Valid log levels are (from most to least verbose): `trace`, `debug`, `info`, `warn`, `error`.
Logs are formatted with a timestamp, target (module), log level and message.

## Notes

* The tool is specifically designed for `dsgvo-portal.de`.  Changes to the website's structure or API may break the tool.
* Always be mindful of the target website's terms of service and robots.txt when scraping data.  The `--delay` option is critical for responsible scraping.
* This tool is intended for personal and educational use only.
* Ensure you have write access to the configured PostgreSQL database.

## Contributing

Contributions, bug reports, and feature requests are welcome! Feel free to open an issue or submit a pull request.