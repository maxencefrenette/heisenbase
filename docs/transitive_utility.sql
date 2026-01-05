ATTACH 'data/heisenbase/index.duckdb' AS hb;

WITH RECURSIVE
edges AS (
  SELECT name, unnest(children) AS child,
         CAST(unknown + win_or_draw + draw_or_loss AS DOUBLE) / list_count(children) AS share
  FROM hb.material_keys
  WHERE list_count(children) > 0
),
prop AS (
  SELECT name, child, share FROM edges
  UNION ALL
  SELECT p.name, unnest(mk.children) AS child,
         p.share / list_count(mk.children) AS share
  FROM prop p
  JOIN hb.material_keys mk ON mk.name = p.child
  WHERE list_count(mk.children) > 0
),
tu AS (
  SELECT child AS material_key, SUM(share) AS transitive_utility
  FROM prop p
  LEFT JOIN hb.material_keys mk ON mk.name = p.child
  WHERE mk.name IS NULL
  GROUP BY child
),
pgn AS (
  SELECT * FROM read_parquet('data/pgn_index.parquet')
)
SELECT
  pgn.*,
  COALESCE(tu.transitive_utility, 0.0) AS transitive_utility,
  1e9 * COALESCE(tu.transitive_utility, 0.0) / pgn.material_key_size AS transitive_utility_norm
FROM pgn
LEFT JOIN tu ON tu.material_key = pgn.material_key;
