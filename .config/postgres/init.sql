CREATE DB svh_db;
CREATE USER svh_user WITH PASSWORD 'SET_YOUR_PASSWORD';
ALTER DATABASE svh_db OWNER TO svh_user;


CREATE TYPE vid_obj_type AS ENUM ('preview', 'orig', 'high', 'low');

CREATE TABLE Users (
    id SERIAL PRIMARY KEY,
    email TEXT NOT NULL,
    name TEXT NOT NULL,
    bot BOOLEAN DEFAULT false,
    cat TIMESTAMP DEFAULT now()
);

CREATE TABLE Videos (
    id SMALLSERIAL PRIMARY KEY,
    kind SMALLINT DEFAULT 0,
    access SMALLINT DEFAULT 0,
    title TEXT,
    description TEXT,
    cat TIMESTAMP DEFAULT now()
);

CREATE TABLE Video_Objects (
    vid SMALLINT REFERENCES Videos(id) ON DELETE CASCADE,
    type vid_obj_type NOT NULL,
    cat TIMESTAMP DEFAULT NOW(),
    PRIMARY KEY (vid, type)
);

CREATE TABLE Transcribe_Jobs (
    vid SMALLINT REFERENCES Videos(id) ON DELETE CASCADE,
    type vid_obj_type NOT NULL,
    cat TIMESTAMP DEFAULT NOW(),
    PRIMARY KEY (vid, type)
);

CREATE TABLE Roles (
    id SMALLSERIAL PRIMARY KEY,
    title TEXT NOT NULL,
    description TEXT,
    cat TIMESTAMP DEFAULT now()
);

CREATE TABLE Tags (
    id SMALLSERIAL PRIMARY KEY,
    title TEXT NOT NULL,
    description TEXT,
    color INTEGER NOT NULL,
    cat TIMESTAMP DEFAULT now()
);

CREATE TABLE Apps (
    id SMALLSERIAL PRIMARY KEY,
    title TEXT NOT NULL,
    description TEXT,
    cat TIMESTAMP DEFAULT now()
);

CREATE TABLE Music (
    id SMALLSERIAL PRIMARY KEY,
    title TEXT NOT NULL,
    description TEXT,
    cat TIMESTAMP DEFAULT now()
);

CREATE TABLE Accesses (
    vid SMALLINT REFERENCES Videos(id),
    uid INT REFERENCES Users(id),
    PRIMARY KEY (vid, uid)
);

CREATE TABLE Video_Tag_Links (
    vid SMALLINT REFERENCES Videos(id),
    tid SMALLINT REFERENCES Tags(id),
    PRIMARY KEY (vid, tid)
);

CREATE TABLE Video_User_Links (
    vid SMALLINT REFERENCES Videos(id),
    uid INT REFERENCES Users(id),
    rid SMALLINT REFERENCES Roles(id),
    PRIMARY KEY (vid, uid)
);

CREATE TABLE Video_App_Links (
    vid SMALLINT REFERENCES Videos(id),
    aid SMALLINT REFERENCES Apps(id),
    PRIMARY KEY (vid, aid)
);

CREATE TABLE Video_Music_Links (
    vid SMALLINT REFERENCES Videos(id),
    mid SMALLINT REFERENCES Music(id),
    PRIMARY KEY (vid, mid)
);

CREATE OR REPLACE FUNCTION get_user_videos(
    p_email TEXT,
    p_limit SMALLINT DEFAULT NULL,
    p_search TEXT DEFAULT NULL,
    p_kind TEXT DEFAULT NULL,
    p_tag INT DEFAULT NULL,
    p_user INT DEFAULT NULL,
    p_app INT DEFAULT NULL,
    is_random BOOLEAN DEFAULT FALSE
)
RETURNS JSON AS $$
DECLARE
    u_id INT;
BEGIN
    SELECT id INTO u_id FROM Users WHERE email = p_email;
    IF u_id IS NULL THEN
        RETURN '[]'::json;
    END IF;

    RETURN (
        SELECT COALESCE(
            json_agg(x ORDER BY
                CASE
                    WHEN is_random AND p_search IS NULL
                    THEN random()
                    ELSE EXTRACT(EPOCH FROM cat)
                END DESC
            ),
            '[]'::json
        ) FROM (
            SELECT v.cat, json_build_object(
                'id', v.id,
                'title', v.title,
                'tags', (
                    SELECT json_agg(
                        json_build_object(
                            'id', t.id,
                            'title', t.title,
                            'color', t.color
                        )
                    )
                    FROM Tags t
                    JOIN Video_Tag_Links tp ON tp.tid = t.id
                    WHERE tp.vid = v.id
                ),
                'users', (
                    SELECT json_agg(
                        json_build_object(
                            'id', u.id,
                            'email', u.email,
                            'name', u.name,
                            'role', r.title
                        )
                    )
                    FROM Video_User_Links up
                    JOIN Users u ON u.id = up.uid
                    JOIN Roles r ON r.id = up.rid
                    WHERE up.vid = v.id
                ),
                'apps', (
                    SELECT json_agg(
                        json_build_object(
                            'id', a.id,
                            'title', a.title
                        )
                    )
                    FROM Apps a
                    JOIN Video_App_Links ap ON ap.aid = a.id
                    WHERE ap.vid = v.id
                )
            ) AS x
            FROM Videos v
            JOIN Video_User_Links up ON up.vid = v.id
            WHERE up.uid = u_id AND (
                p_search IS NULL
                OR
                v.title ILIKE '%' || p_search || '%'
            ) AND (
                p_kind IS NULL
                OR
                v.kind = CASE p_kind
                    WHEN 'full' THEN 0
                    WHEN 'clip' THEN 1
                    WHEN 'short' THEN 2
                    ELSE v.kind
                END
            ) AND (
                p_tag IS NULL
                OR
                EXISTS (
                    SELECT 1 FROM Video_Tag_Links tp
                    WHERE tp.vid = v.id AND tp.tid = p_tag
                )
            ) AND (
                p_user IS NULL
                OR
                EXISTS (
                    SELECT 1 FROM Video_User_Links up2
                    WHERE up2.vid = v.id AND up2.uid = p_user
                )
            ) AND (
                p_app IS NULL
                OR
                EXISTS (
                    SELECT 1 FROM Video_App_Links ap
                    WHERE ap.vid = v.id AND ap.aid = p_app
                )
            )
            LIMIT p_limit
        )
    );
END;
$$ LANGUAGE plpgsql STABLE;

CREATE OR REPLACE FUNCTION get_meta()
RETURNS jsonb AS $$
BEGIN
    SELECT jsonb_build_object(
        'users', (
            SELECT jsonb_agg(jsonb_build_object('id', u.id, 'name', u.name))
            FROM Users u
        ),
        'roles', (
            SELECT jsonb_agg(jsonb_build_object('id', r.id, 'title', r.title))
            FROM Roles r
        ),
        'apps', (
            SELECT jsonb_agg(jsonb_build_object('id', a.id, 'title', a.title))
            FROM Apps a
        ),
        'tags', (
            SELECT jsonb_agg(jsonb_build_object('id', t.id, 'title', t.title))
            FROM Tags t
        )
    );
END;
$$ LANGUAGE plpgsql STABLE;

CREATE OR REPLACE FUNCTION get_video_meta(p_vid INT)
RETURNS jsonb AS $$
BEGIN
    SELECT jsonb_build_object(
        'title', v.title,
        'description', v.description,
        'users', COALESCE((
            SELECT jsonb_agg(jsonb_build_object('uid', up.uid, 'rid', up.rid))
            FROM Video_User_Links up
            WHERE up.vid = p_vid
        ), '[]'::jsonb),
        'apps', COALESCE((
            SELECT jsonb_agg(ap.aid)
            FROM Video_App_Links ap
            WHERE ap.vid = p_vid
        ), '[]'::jsonb),
        'tags', COALESCE((
            SELECT jsonb_agg(tp.tid)
            FROM Video_Tag_Links tp
            WHERE tp.vid = p_vid
        ), '[]'::jsonb)
    )
    FROM Videos v
    WHERE v.id = p_vid;
END;
$$ LANGUAGE plpgsql STABLE;

CREATE OR REPLACE FUNCTION get_trn_job()
RETURNS TABLE (job_vid SMALLINT, job_type vid_obj_type) AS $$
DECLARE
    found RECORD;
BEGIN
    SELECT v.id AS vid, t.type
    INTO found
    FROM Videos v
    CROSS JOIN unnest(enum_range(NULL::vid_obj_type)) AS t(type)
    WHERE t.type != 'orig'
      AND NOT EXISTS (
          SELECT 1 FROM Video_Objects vo
          WHERE vo.vid = v.id AND vo.type = t.type
      )
      AND NOT EXISTS (
          SELECT 1 FROM Transcribe_Jobs tj
          WHERE tj.vid = v.id AND tj.type = t.type
      )
    ORDER BY v.cat
    LIMIT 1;

    IF found IS NULL THEN
        RETURN;
    END IF;

    INSERT INTO Transcribe_Jobs (vid, type)
    VALUES (found.vid, found.type)
    ON CONFLICT (vid, type) DO NOTHING;

    IF NOT FOUND THEN
        RETURN;
    END IF;

    job_vid := found.vid;
    job_type := found.type;
    RETURN NEXT;
END;
$$ LANGUAGE plpgsql;

CREATE OR REPLACE FUNCTION complete_trn_job(p_vid SMALLINT, p_type vid_obj_type)
RETURNS VOID AS $$
BEGIN
    INSERT INTO Video_Objects (vid, type)
    VALUES (p_vid, p_type)
    ON CONFLICT (vid, type) DO NOTHING;

    DELETE FROM Transcribe_Jobs
    WHERE vid = p_vid AND type = p_type;
END;
$$ LANGUAGE plpgsql;

