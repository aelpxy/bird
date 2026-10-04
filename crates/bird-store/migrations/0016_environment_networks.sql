-- NULL keeps the network birdd was started with, which environments created before this used
ALTER TABLE environments ADD COLUMN network TEXT;
CREATE UNIQUE INDEX environment_networks ON environments (network);
