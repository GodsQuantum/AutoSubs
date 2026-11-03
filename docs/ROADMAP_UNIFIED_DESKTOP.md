# AutoSubs — plan complet d'une application multiplateforme avec moteurs facultatifs

**Checkpoint : 3 novembre 2026.** Dépôt canonique : Applications/AutoSubs/repo, worktree documentaire isolé à partir de origin/main 83adc27. Le vrai point d'état métier reste le fichier parent Applications/AutoSubs/handoffs/HANDOFF_CURRENT.md. Ceci est un plan de réalisation jusqu'aux releases ; ne pas déclarer les futures fonctions installées.

## Situation réelle : à préserver

- AutoSubs 3.6.3 : logiciel de sous-titres et rendus vidéo, éditeur et workflows, service CT300 existant. Source actuelle de production pour la correction d'exclusivité des sous-titres = PR #84 et image hotfix, PAS la main GitHub.
- PR #84 fix/exclusive-ass-dialogues-20261102, HEAD 3386e9b : toujours OPEN/BLOCKED à la vérification du 3 novembre. Ne pas contourner la protection de branche ; la faire passer en mettant à niveau proprement les dépendances et le packaging seulement quand les sources vérifiées le permettent.
- Le dépôt local AutoSubs/repo contient deux MODIFICATIONS PRÉEXISTANTES non commitées dans tools/standup_corpus.py et tools/systemd/autosubs-standup-corpus.service. Ne ni effacer, ni reset, ni ajouter à un commit hors périmètre ; travailler en worktree depuis origin/main.
- STT/alignement prod : Cloud9-Speaches partagé en CT410, compatible OpenAI ; ce fournisseur doit rester le défaut serveur, et les fallback existants restent fonctionnels. Ne pas recréer trois conteneurs auxiliaires supprimés.
- Le frontend de l'application web/native AutoSubs fonctionne déjà sous plusieurs OS avec ses propres fonctions sous-titres (cut/preview/ASS/SRT/FFmpeg). Le launcher AutoSubs ne possède actuellement PAS la gestion systray complète de ScribeWatch ; ne pas écrire que c'est fait.

## Architecture cible (pas de deuxième application)

Le serveur Rust AutoSubs, son frontend Svelte et son pipeline sous-titrage restent une seule source de vérité. Le packaging desktop est une enveloppe de cette application sur Windows x64, macOS Intel/ARM, Linux x64/ARM64. L'interface est la même partout, dans le navigateur et dans le desktop. Ne pas créer une seconde UI desktop ni copier tout ScribeWatch.

Créer un composant de gestion de moteurs compatible avec les APIs communes existantes :
- Transcription/alignement : Interne / Externe / Désactivé. Interne => Cloud9-Speaches natif dans le bundle et disponible en local loopback ; externe => URL transcriptions + endpoint d'alignement distinct (Cloud9 local, autre LAN ou serveur distant), sans lancer de nouveau service. Désactivé => pas de STT ; l'édition/encodage à partir de sous-titres importés restent disponibles.
- LLM facultatif : Interne / Externe / Désactivé. Interne => llama.cpp router natif si tâches de traduction/reformulation/rédaction de métadonnées le justifient, sinon aucun démarrage, ni réservation VRAM. Externe => endpoint existant OpenAI-compatible. Les sous-titres sourcés doivent rester canonisés ; aucune réécriture silencieuse.
- Liste des modèles installés et modèles téléchargeables : déléguer au service Cloud9-Speaches via API, réutiliser son cache Hugging Face GGUF, quantification choisie. Pour LLM, déléguer au routeur natif llama.cpp /models, sans téléchargeur parallèle personnalisé. Import éventuel de URL Hugging Face via contrôles de sécurité déjà vérifiés.
- UN seul propriétaire pour chaque subprocess, ports 127.0.0.1 réservés, conflits détectés avant spawn. Ne jamais tuer un fournisseur externe. Les modèles ne téléchargent pas automatiquement et les secrets restent côté backend.

## Phases d'implémentation et conditions d'acceptation

### 0. Stabiliser la base existante

1. Audit read-only branch/main, PR84, Cargo.lock et npm lock, FFmpeg/libass, tests de synchronisation, systray absent et préparation d'un rollback fiable.
2. Réparer CI requise de PR84 : rustls-webpki, racines de confiance, deepmerge si correction upstream disponible, et SHA d'outils Linux AppImage sur références officielles. Ne pas désactiver les audits ni accepter des hashes non vérifiés ; éventuellement isoler en PR de maintenance.
3. Relancer les vrais tests exclusivité ASS + un rendu vidéo court synthétique et obtenir merge après CI, sans toucher au process CT300 avant preuve.
4. Réconcilier main GitHub avec production hotfix ; conserver la configuration et les sorties Reels actuelles, sans rediriger Reels_Subbed ni déclencher AutoPublisher par inadvertance.

### 1. Systray et mono-instance

1. Étudier packaging/desktop/launcher/src/main.rs d'AutoSubs et packaging/desktop/launcher/src/desktop_tray.rs de ScribeWatch.
2. Porter le composant natif minimal, pas copier l'app entière. Menu : ouvrir AutoSubs, état des jobs, mettre en pause/reprendre watchers, accéder au dossier sortie, afficher les logs, arrêter moteurs internes, quitter. Fermeture/minimisation configure ce qui reste vivant.
3. Tester re-login, absence de double instance, collision de port, crash/relaunch, orphan processes, interrupteur Run at login explicitement opt-in. UI ne doit pas disparaître sans moyen de rouvrir.
4. Adapter icônes et menus aux directives OS ; tester headless/offline et sessions utilisateur sans droit admin.

### 2. Deux moteurs intégrés facultatifs

1. Utiliser les mêmes contrats API d'état que ScribeWatch et les routes existantes AutoSubs, sans imposer une base/structure JSON incompatibles. Construire une couche d'adaptation AutoSubs plutôt que faire un fork de Cloud9-Speaches.
2. STT : inclure le binaire Cloud9-Speaches certifié par architecture ; modèle de base Whisper Turbo, alternatives Parakeet/Qwen ASR/Nemotron réellement compatibles. Alignment via CTC existant et tests d'horodatage ; fournir des endpoints configurables pour hôtes externes.
3. LLM : inclure llama.cpp upstream pour les traitements IA seulement (traduction/synthèse éventuelle). Respecter les entrées/sorties sous-titres natives et ne pas ajouter de résumés lorsque l'utilisateur n'en demande pas. Configurer une limite d'un modèle actif, marge de VRAM et sleep-idle ; pas de préchargement.
4. Coordination VRAM : AutoSubs utilise déjà FFmpeg (et potentiellement GPU NVENC/Vulkan). Ajouter admission mémoire avant chargement STT/LLM et éviter deux charges GPU concurrentes par défaut. Priorité au rendu vidéo et à l'alignement correct. Mesurer le temps sous charge.
5. Préserver une alternative CPU et toutes les chaînes d'external providers, sans secrets côté UI. Test de bascule interne/externe sans effacer les presets existants.

### 3. Paquets natifs par système

- Windows x64 : un seul installeur EXE, archives sources épinglées SHA256, tous les runtimes/FFmpeg/FFprobe/licences/manifestes. Utiliser le système de ZIP intégré de ScribeWatch si le poids brut dépasse la limite stock NSIS 2 GiB ; tester installation véritable, exécution, arrêt, désinstallation et rollback.
- macOS Intel/ARM : binaires STT Metal/CPU et llama.cpp Metal, .app/DMG, frameworks signables sans dépendances à Homebrew. Signatures récursives et notarisation quand les identifiants seront disponibles.
- Linux x64/ARM64 : AppImage CPU/Vulkan, dépendances ldd/readelf auditées, FUSE ou mode extraction ; pas de daemon root. Toujours même frontend, mêmes menus et mêmes statuts API.
- Dépendances distribuées sous licence et manifest de provenance. Model weights hors paquet ; download uniquement après consentement. Produire des scripts stage déterministes testables, pas de bricolages manuels.
- CI 5 matrices : binaire réel, service local STT et LLM, endpoints /health et /models, transcodage FFmpeg/libass de vidéo synthétique, export ASS/SRT/MP4, systray en session graphique si runner possible, stop/uninstall, SHA256 de chaque artefact.

### 4. Validation fonctionnelle, performance, sécurité

- Tests métier : importer SRT/ASS, découper vidéo, aligner mots, éviter chevauchement des dialogues, garder les titres éditoriaux source, rendu 1080x1920 et 16:9, absence de sous-titres redondants, audio long segmenté.
- Résilience : échec fournisseur et fallback, pause/reprise, erreur de disque, crash d'un modèle interne, RAM/VRAM saturée, arrêt propre des enfants, pas de perte d'un rendu déjà publié.
- Contrôle Git et secrets : ne jamais publier de vrais noms/fichiers clients, logs redigés, origines navigateur vérifiées, endpoints loopback pour toute action native d'installation, no arbitrary installer URLs.
- Benchmark machines réelles Windows GTX1650 4Go et RTX3070 8Go, Linux AMD 780M, macOS Apple Silicon ou Intel selon disponibilité : même modèle et sortie, peak VRAM et timing, surveillance encodage FFmpeg en parallèle, absence de conflit avec ScribeWatch et Handy.
- Staging isolé CT700, image rollback CT300 conservée, monitoring santé et traitement réel avant promotion.

### 5. Signature et publication

Au 3 novembre 2026, GitHub Actions AutoSubs présente zéro secret de signature et les répertoires projet ne contiennent aucun certificat p12/pfx de signature. Ne pas appeler « signés » des binaires auto-signés ou seulement hashés.

- Pour Windows : identité Authenticode OV/EV ou Azure Trusted Signing dans CI avec accès restrictif, signtool, timestamp et lecture Get-AuthenticodeSignature sur artefact publié.
- Pour macOS : Apple Developer ID, codesign, hardened runtime, notarisation, stapling et vérification sur machine neuve.
- Pour Linux : fichiers SHA256 et signatures détachées minisign/GPG avec clé d'éditeur.
- Ne donner accès aux secrets qu'au workflow de release protégé, jamais PR de contributeur. Chaque signature change la somme finale : recalculer, revérifier, publier de nouveaux assets ou une release clairement versionnée.
- Versions finales et docs : même interface, systray, moteurs facultatifs, réglages persistants, tests exacts par OS, changelog, guides installation et dépannage, instructions rollback. Laisser les versions slim disponibles.

## Dépendances et ordre réel

PR84 verte et intégrée → launcher/tray certifiés → états API STT/LLM → composants natifs et gestion mémoire → bundler par OS → tests import/rendu vidéo réel et modèles → signatures de l'éditeur → releases multi-OS → retour terrain et nettoyage des worktrees vérifiés. Les phases peuvent être développées en parallèle en branches séparées, jamais être déclarées terminées avant les gates ci-dessus.

### Politique de confidentialité des exemples

Toutes les fixtures et captures de test doivent être génériques et synthétiques. Aucun nom, date de naissance, dossier client ou document clinique présent dans les captures d'une autre application n'est autorisé dans AutoSubs, ses captures ou ses releases.
