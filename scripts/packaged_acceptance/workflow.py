"""A visible product workflow with independent read-only durable-state witnesses."""
import json

from native_release.files import inventory
from .exports import freeze, verify

REQUIRED = ('setup', 'language', 'import', 'metadata-artwork', 'inspection', 'broken-undo',
            'tags-search', 'collections', 'shelf-create', 'shelf-review',
            'export-results', 'export-selection', 'export-collections', 'export-shelves',
            'history', 'trash-removal', 'relocation', 'reopen', 'scaling', 'offline-startup',
            'recovery', 'uninstall-reinstall')


def perform(driver, fixture, result, french):
    def words(en, fr):
        return fr if french else en
    def observe(name, value):
        if name not in REQUIRED or name in result['checks']:
            raise ValueError('unexpected/duplicate packaged acceptance checkpoint')
        driver.snapshot(name)
        result['checks'][name] = value
    def closed():
        driver.press(words('Close', 'Fermer'))
    def terminal(kind, previous):
        def completed():
            rows = [row for row in fixture.rows("SELECT * FROM file_operations WHERE kind='"+kind+"' AND finished_at IS NOT NULL")
                    if row['id'] not in previous]
            if len(rows) > 1:
                raise ValueError('multiple new terminal operations cannot identify the GUI action')
            return rows[0] if rows else None
        return driver.wait(completed, timeout=40)
    # Both fresh setup and Go-era independent-copy adoption use the real native
    # picker and explicit confirmation. The source fixture remains immutable.
    driver.wait(lambda: driver.find('Choose library folder…') or driver.find('Set up…'))
    if not driver.find('Choose library folder…'):
        driver.press('Set up…')
    driver.press('Choose library folder…')
    driver.picker(fixture.candidate)
    driver.press('Create an independent copy' if fixture.legacy else 'Create library in this folder')
    driver.wait(lambda: fixture.current_root() == fixture.candidate)
    observe('setup', {'root': str(fixture.current_root()), 'independent_adoption': fixture.legacy is not None})
    if french:
        driver.press('Français')
        driver.wait(lambda: json.loads((fixture.native_profile/'settings.json').read_text())['language'] == 'fr')
    observe('language', {'persisted': json.loads((fixture.native_profile/'settings.json').read_text())['language']})
    closed()
    previous = {row['id'] for row in fixture.rows("SELECT id FROM file_operations")}
    driver.press(words('Import…', 'Importer…'))
    driver.press(words('Choose archives…', 'Choisir des archives…'))
    driver.picker(fixture.inputs, files=True)
    imported = terminal('import', previous)
    assert fixture.rows("SELECT uuid FROM stories WHERE uuid='00112233-4455-4677-8899-aabbccddeeff'")
    observe('import', {'terminal': imported, 'items': fixture.rows(
        'SELECT source_name,status,outcome_code FROM file_operation_items'), 'managed':fixture.archives(reviewed=False)})
    closed()
    # The adopted library contains more rows than the viewport. Search through
    # the public UI so the imported story is mounted before selecting it.
    driver.text(words('Search stories', 'Rechercher des histoires'), 'Clockwork')
    driver.press(words('Sync optional official metadata', 'Synchroniser les métadonnées officielles · facultatif'))
    driver.wait(lambda: fixture.rows("SELECT id FROM catalog_syncs WHERE status='succeeded'"), timeout=40)
    driver.wait(lambda: fixture.rows('SELECT id FROM official_story_metadata'), timeout=30)
    driver.press('The Clockwork Mountain', prefix=True)
    driver.wait(lambda: fixture.rows('SELECT id FROM catalog_artworks WHERE cached_at IS NOT NULL'), timeout=30)
    observe('metadata-artwork', {'sync': fixture.rows('SELECT id,status,matched_story_count FROM catalog_syncs'),
                                'artwork': fixture.rows('SELECT id,sha256,byte_size,managed_path FROM catalog_artworks')})
    driver.press(words('Details…', 'Détails…'))
    driver.key('End')
    driver.press(words('Show technical details', 'Afficher les détails techniques'))
    observe('inspection', {'uuid': '00112233-4455-4677-8899-aabbccddeeff'})
    driver.key('Home')
    driver.press(words('Broken · ', 'Défectueuse · '), prefix=True)
    driver.wait(lambda: fixture.rows("SELECT id FROM stories WHERE uuid='00112233-4455-4677-8899-aabbccddeeff' AND is_broken=1"))
    driver.press(words('Undo Broken change', 'Annuler le changement de statut'))
    driver.wait(lambda: fixture.rows("SELECT id FROM stories WHERE uuid='00112233-4455-4677-8899-aabbccddeeff' AND is_broken=0"))
    observe('broken-undo', {'restored': True})
    closed()
    driver.press(words('Tags', 'Étiquettes'))
    driver.choose(words('Tag to edit', 'Étiquette à modifier'), 0)
    driver.text(words('Immutable tag key', 'Clé immuable de l’étiquette'), 'packaged-test')
    driver.text(words('Tag label', 'Nom de l’étiquette'), 'Été packaged')
    driver.text(words('Color · #RRGGBB', 'Couleur · #RRGGBB'), '#123456')
    driver.press(words('Create tag', 'Créer l’étiquette'))
    tag = driver.wait(lambda: fixture.rows("SELECT id,key,label FROM tag_definitions WHERE key='packaged-test'"))
    closed()
    driver.text(words('Search stories', 'Rechercher des histoires'), 'Clockwork')
    driver.wait(lambda: driver.find('The Clockwork Mountain', prefix=True))
    driver.press(words('Details…', 'Détails…'))
    driver.press(words('Edit tags', 'Modifier les étiquettes'), prefix=True)
    driver.choose('Été packaged', 1, prefix=True)
    assignments = driver.wait(lambda: fixture.rows(
        "SELECT a.story_id,a.definition_id FROM story_tag_assignments a JOIN tag_definitions d ON d.id=a.definition_id WHERE d.key='packaged-test'"))
    observe('tags-search', {'created': tag, 'assigned': assignments, 'native_utf8_text': 'Été packaged', 'search': 'Clockwork'})
    closed()
    closed()
    driver.text(words('Search stories', 'Rechercher des histoires'), '')
    driver.press(words('Manage Collections…', 'Gérer les Collections…'))
    driver.choose(words('Collection to manage', 'Collection à gérer'), 0)
    driver.text(words('Collection name', 'Nom de la Collection'), 'Packaged all')
    driver.press(words('Save current filters as Collection', 'Enregistrer les filtres actuels comme Collection'))
    collections = driver.wait(lambda: fixture.rows("SELECT id,name,position FROM collections WHERE name='Packaged all'"))
    observe('collections', {'saved': collections})
    closed()
    driver.focus(words('All stories', 'Toutes les histoires'), roles=('list box', 'AXList'))
    driver.key('End')
    driver.press(words('Select · ', 'Sélectionner · ')+'The Clockwork Mountain', prefix=True)
    driver.press(words('Manage Shelves…', 'Gérer les Étagères…'))
    driver.choose(words('Shelf to manage', 'Étagère à gérer'), 0)
    driver.text(words('Shelf name', 'Nom de l’Étagère'), 'Packaged approved')
    sources = fixture.rows('SELECT id,name FROM collections ORDER BY position,id')
    driver.choose(words('Source Collection', 'Collection source'), next(i for i,row in enumerate(sources) if row['name']=='Packaged all'))
    driver.press(words('Include this source', 'Inclure cette source'))
    driver.press(words('Preview initial members…', 'Examiner les membres initiaux…'))
    driver.press(words('Select all candidates', 'Tout sélectionner'))
    expected = len(fixture.rows('SELECT uuid FROM stories'))
    driver.press(words('Create ', 'Créer ')+str(expected))
    driver.wait(lambda: fixture.rows("SELECT id FROM shelves WHERE name='Packaged approved'"))
    members = fixture.rows("SELECT story_uuid,position FROM shelf_stories WHERE shelf_id=(SELECT id FROM shelves WHERE name='Packaged approved')")
    assert len(members) == expected
    observe('shelf-create', {'approved': members})
    # Add a new actual archive after establishing a linked source. A review may
    # suggest it but cannot approve it until the visible explicit Apply action.
    driver.press(words('Import…', 'Importer…'))
    previous = {row['id'] for row in fixture.rows("SELECT id FROM file_operations")}
    driver.press(words('Choose archives…', 'Choisir des archives…'))
    driver.picker(fixture.review_inputs, files=True)
    terminal('import', previous)
    closed()
    driver.press('Packaged approved')
    driver.choose(words('Shelf action', 'Action sur l’Étagère'), 7)
    driver.press(words('Run', 'Exécuter'))
    before = fixture.rows("SELECT story_uuid FROM shelf_stories WHERE shelf_id=(SELECT id FROM shelves WHERE name='Packaged approved')")
    assert fixture.review_uuid not in {row['story_uuid'] for row in before}
    review = driver.wait(lambda: next((node for node in driver.snapshot()['nodes']
                                      if node.get('role') in ('combo box', 'AXComboBox', 'AXPopUpButton')
                                      and fixture.review_uuid in (node.get('name','')+node.get('title','')+node.get('description',''))), None))
    name = review.get('name') or review.get('title') or review.get('description')
    driver.choose(name, 1)
    driver.press(words('Apply approved changes', 'Appliquer les modifications approuvées'), prefix=True)
    driver.wait(lambda: fixture.rows("SELECT story_uuid FROM shelf_stories WHERE story_uuid='"+fixture.review_uuid+"'"))
    observe('shelf-review', {'before': before, 'after': fixture.rows("SELECT story_uuid,position FROM shelf_stories WHERE shelf_id=(SELECT id FROM shelves WHERE name='Packaged approved')")})
    driver.press(words('Not now · close review', 'Pas maintenant · fermer la révision'))
    archives = fixture.archives(reviewed=True)
    all_uuids = {row['uuid'] for row in archives}
    shelf = fixture.rows("SELECT id,name FROM shelves WHERE name='Packaged approved'")
    collection = fixture.rows("SELECT id,name FROM collections WHERE name='Packaged all'")
    approved = {row['story_uuid'] for row in fixture.rows(
        "SELECT story_uuid FROM shelf_stories WHERE shelf_id=(SELECT id FROM shelves WHERE name='Packaged approved')")}
    assert approved == all_uuids
    for index, scope in enumerate(('results', 'selection', 'collections', 'shelves')):
        if scope == 'results':
            driver.text(words('Search stories', 'Rechercher des histoires'), 'Clockwork')
        if scope == 'selection' and not driver.find(words('Deselect · ', 'Désélectionner · ')+'The Clockwork Mountain', prefix=True):
            driver.press(words('Select · ', 'Sélectionner · ')+'The Clockwork Mountain', prefix=True)
        driver.press(words('Export…', 'Exporter…'))
        driver.choose(words('Export scope', 'Portée de l’export'), index)
        if index in (2,3):
            driver.press('Packaged all' if index == 2 else 'Packaged approved', prefix=True)
        destination = fixture.exports/scope
        destination.mkdir()
        uuids = {'00112233-4455-4677-8899-aabbccddeeff'} if index < 2 else all_uuids
        expectation = freeze(archives, fixture.current_root(), uuids,
                             ('current_query','selection','collection','curated_shelf')[index],
                             collection if index == 2 else shelf if index == 3 else [], destination)
        driver.press(words('Choose', 'Choisir'), prefix=True)
        driver.picker(destination)
        previous = {row['id'] for row in fixture.rows('SELECT id FROM file_operations')}
        driver.press(words('Export eligible archives', 'Exporter les archives éligibles'))
        operation = terminal('export', previous)
        exported = inventory(destination)
        items = fixture.rows("SELECT * FROM file_operation_items WHERE operation_id='"+operation['id']+"' ORDER BY id")
        observation = verify(expectation, operation, items, exported)
        if scope == 'results':
            driver.press(words('Reveal destination', 'Afficher la destination'))
            observation['actual_file_manager'] = driver.reveal(destination, exported)
        observe('export-'+scope, observation)
        closed()
        if scope == 'results':
            driver.text(words('Search stories', 'Rechercher des histoires'), '')
    driver.press(words('History', 'Historique'))
    observe('history', {'terminal': fixture.rows('SELECT id,kind,status,finished_at FROM file_operations ORDER BY id')})
    closed()
    return observe
