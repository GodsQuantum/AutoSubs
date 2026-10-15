# Corpus stand-up Cloud9 — AutoSubs v3.6.0

## Périmètre

- Sources inchangées: `/srv/storage/production/active/Montage/Sources/Stand-Up/`, recherche récursive.
- Aucun archivage, déplacement, destruction ni réencodage des vidéos originales.
- Trois fichiers générés à côté de chaque vidéo: `<nom>.md`, `<nom>.srt`, `<nom>.words.json`.
- Même moteur et aligneur qu'AutoSubs prod: STT Cloud9-Speaches CT410 192.168.1.219:8005 et WhisperX fr CT410 192.168.1.219:8006.
- L'ASR ne garantit pas les mots prononcés: les erreurs de reconnaissance nécessitent une relecture avant publication. Le forced-alignment fournit de véritables bornes mot par mot quand valide, sinon le fichier n'est pas publié.
- Sous-titres SRT groupés pour lisibilité, JSON conservant tous les mots/timestamps d'origine, Markdown déterministe avec horodatages et chemin original, pas de LLM réécrivant les propos.
- Durées absolues de la vidéo source, audio segmenté en 240 secondes, nettoyage audio intermédiaire; reprise SQLite par segment sauvegardé.
- FTS5 insensible aux accents de base, recherche exacte de mots ou séquence de mots (pas recherche sémantique de synonymes).
- Deux vues/prises contenant un audio similaire sont transcrites séparément, aucun regroupement risqué de captations.
- Dossier et fichiers de sauvegarde du workflow séparés des médias. Pas de doublon massif de vidéo.

## Service de production sur hôte Cloud9

Service systemd **persistant** activé le 15/10/2026: `autosubs-standup-corpus.service`, lancé au démarrage de Cloud9 et reprenant les chunks validés. Le fichier installé est suivi dans `tools/systemd/autosubs-standup-corpus.service`.
N'importe quel démarrage ultérieur `run` ignore les vidéos possédant déjà les trois sidecars et reprend les segments enregistrés dans SQLite.

```bash
systemctl status autosubs-standup-corpus.service --no-pager
journalctl -u autosubs-standup-corpus.service -f
python3 '/srv/storage/nvme/DEV/Projets/Ssker Content Production/AutoSubs/repo/tools/standup_corpus.py' stats
python3 '/srv/storage/nvme/DEV/Projets/Ssker Content Production/AutoSubs/repo/tools/standup_corpus.py' search 'politique'
```

Recherche renvoie chemin relatif de la vidéo, timecode de début et texte concordant. Exemple utilisable par Archie/Hermes via commande locale; ouvrir la vidéo indiquée et avancer au timecode. Des mots situés de part et d'autre d'une fenêtre de 30 secondes ne seront pas détectés en recherche d'expression.

Arrêt contrôlé de batch: `systemctl stop autosubs-standup-corpus.service`. Un chunk non validé peut devoir être retranscrit. Ne pas tuer les moteurs partagés CT410.

Les fichiers `.md`, `.srt`, `.words.json` déjà présents ne sont jamais écrasés. Pour corriger une transcription, privilégier un workflow de relecture; ne pas effacer de sidecars ni l'index sans copie/sauvegarde.

## Découpage dans AutoSubs

Ouvrir un job vidéo dans l'Éditeur, cliquer sur «Début ici», puis «Fin ici» pendant la lecture et «+ Garder ce passage». Répéter pour d'autres plages. Vérifier la liste des intervalles, cliquer «Cut». Le moteur FFmpeg produit un nouveau `/data/cuts/<nom>-cut-<id>.mp4`; télécharger via le lien de l'interface. L'original et ses sidecars ne sont jamais modifiés. Les intervalles doivent être triés, non superposés, dans la durée du média; limite de 32. L'export utilise une seule tâche CPU à la fois, des points de coupe précis et une sortie H.264/AAC. Le statut serveur de cette première version est en mémoire: un redémarrage du backend peut effacer le lien de téléchargement même si le MP4 demeure sur disque. Les clips produits conservent du son et une durée recollée, sans sous-titres brûlés automatiquement.

## Tests vérifiés au 15/10/2026

- 240 secondes de captation Paname, 608 mots réels alignés, source non modifiée.
- Un clip bref réel du dossier Divers a produit .md/.srt/.words.json et un index texte (le bruit musical peut provoquer des hallucinations de ponctuation, désormais filtrées).
- 122 tests unitaires Rust; 37 tests frontend Node; 3 tests Python corpus.
- `svelte-check`: 0 erreur / 0 avertissement.
- Deux coupes 1–2 s et 4–6 s d'une source FFmpeg synthétique donnent un MP4 de 3.000 s avec vidéo et audio.
- Les 52 vidéos du corpus ne sont **pas** certifiées complètes tant que le service n'a pas fini et les fichiers ne sont pas contrôlés.
