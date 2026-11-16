/**
 * Version de l'addon montrée au joueur : « 1.0 », sans le numéro que le site ajoute quand il
 * régénère la table des composants (1.0.3). Les comparaisons (côté Rust) gardent la version entière.
 */
export function versionLabel(version: string | null | undefined): string | null {
    if (!version) return null;
    const m = version.match(/^(\d+\.\d+)(?:\.\d+)*(-.+)?$/);
    return m ? m[1] + (m[2] ?? '') : version;
}
