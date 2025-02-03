CREATE TABLE IF NOT EXISTS incidents (
     incident_id INTEGER PRIMARY KEY,
     org_publish_date DATE NOT NULL,
     modified_date TIMESTAMP WITH TIME ZONE NOT NULL,
     published INTEGER NOT NULL,
     publish_date TIMESTAMP WITH TIME ZONE NOT NULL,
     affected_obj TEXT NOT NULL,
     affected_type TEXT NOT NULL,
     country TEXT NOT NULL,
     details_text TEXT NOT NULL,
     tags TEXT NOT NULL,
     href TEXT NOT NULL,
     "references" JSONB NOT NULL,
     incident_text TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS incident_history (
    id SERIAL PRIMARY KEY,
    content JSONB NOT NULL,
    created_at TIMESTAMP WITH TIME ZONE DEFAULT CURRENT_TIMESTAMP
);