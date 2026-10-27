"""Native workflow review: real keyboard -> AT-SPI -> Orca -> captured PCM."""
import argparse
import hashlib
import json
import os
from pathlib import Path
import signal
import time
import traceback
import wave
import desktop as native
import speech

# Independent expected native names. Domain names (Bedtime) remain user content.
CASES = {
 'browse': ('Search stories','Rechercher des histoires'),
 'filters': ('Search filters','Rechercher un filtre'),
 'details': ('Show technical details','Afficher les détails techniques'),
 'removal': ('Keep story','Conserver l’histoire'),
 'tag-manager': ('Tag label','Nom de l’étiquette'),
 'tag-assignment': ('Bedtime','Bedtime'),
 'tag-delete': ('Cancel','Annuler'),
 'flag-bulk': ('Broken state ·','Statut Défectueuse ·'),
 'collections': ('Collection name','Nom de la Collection'),
 'collection-union': ('Save a copy of the union with this name','Enregistrer une copie de l’union avec ce nom'),
 'collection-impact': ('Cancel','Annuler'),
 'shelf-members': ('Shelf action','Action sur l’Étagère'),
 'shelf-create': ('Cancel','Annuler'),
 'shelf-exclusions': ('Refresh exclusions','Actualiser les exclusions'),
 'review-additions': ('Refresh and renew decisions','Actualiser et renouveler les décisions'),
 'review-removals': ('Refresh and renew decisions','Actualiser et renouveler les décisions'),
 'review-delete': ('Not now · close review','Pas maintenant · fermer la révision'),
 'import': ('Refresh','Actualiser'),
 'sync': ('Refresh local status','Actualiser le statut local'),
 'export': ('Export eligible archives','Exporter les archives éligibles'),
 'export-report': ('Refresh','Actualiser'),
 'history': ('Refresh history','Actualiser l’historique'),
 'history-error': ('Try again','Réessayer'),
 'setup': ('Create library in this folder','Créer la bibliothèque dans ce dossier'),
 'settings': ('English','English'),
 'relocation': ('Move the complete library','Déplacer toute la bibliothèque'),
 'recovery': ('Retry recovery','Réessayer la récupération'),
 'schema': ('Keep files unchanged','Conserver sans modifier'),
}

def digest(path):
    h=hashlib.sha256()
    with path.open('rb') as stream:
        while data:=stream.read(1024*1024): h.update(data)
    return h.hexdigest()

def read_state(path):
    try: return json.loads((path/'state.json').read_text())
    except FileNotFoundError: return None

def run_case(desktop, binary, case, language, mark, announced, pcm, report):
    name=case+'-'+language; output=desktop.root/name; output.mkdir()
    (output/'.librairii-native-fixture').write_text('native-foundations-14\n')
    app=desktop.start(name,[str(binary),str(output),'--workflow-case='+case,'--workflow-language='+language])
    result={'name':name,'pid':app.pid,'navigation':[]};report['cases'].append(result)
    def alive():
        if app.poll() is not None: raise RuntimeError(name+' exited: '+(desktop.root/(name+'.log')).read_text()[-5000:])
        return read_state(output)
    def state(): return native.wait_for(alive)
    try:
        initial=native.wait_for(lambda: (s:=alive()) and s['french']==(language=='fr') and s)
        native.wait_for(lambda: desktop.active_pid()==app.pid)
        accessible=native.wait_for(lambda: desktop.find(lambda n:n.getRole()==desktop.a.ROLE_APPLICATION and n.get_process_id()==app.pid))
        def tree(): return speech.tree(desktop,accessible)
        def snapshot(label):
            nodes=tree();(output/(label+'.json')).write_text(json.dumps(nodes,ensure_ascii=False,indent=2)+'\n')
            result[label]={'focused':speech.focused(nodes),'state':state(),'audio_offset':pcm.stat().st_size}
            desktop.screenshot(output/(label+'.png'))
            return nodes
        def key(value):
            assert desktop.active_pid()==app.pid, name+' lost foreground keyboard target'
            desktop.key(value);time.sleep(.08)
        def quiet():
            # Do not count earlier speech as this control's audio witness.
            native.wait_for(lambda: pcm.stat().st_size>=16000 and speech.audio_facts(pcm.read_bytes()[-16000:])['nonzero_samples']==0)
        def focus(label):
            for _ in range(100):
                current=speech.focused(tree());result['navigation'].append(current)
                if len(current)==1 and current[0]['name'].startswith(label):return current[0]
                key('Tab')
            raise AssertionError(name+' keyboard target unreachable: '+label)
        def announce_focus(step, label, expected):
            target=focus(label)
            assert target['states']['enabled'] and target['states']['sensitive'],name+' target disabled'
            for leave,back in [('shift+Tab','Tab'),('Tab','shift+Tab')]:
                key(leave)
                def neighbor_ready():
                    observed=speech.focused(tree())
                    if len(observed)>1:
                        result['ambiguous_focus']=observed
                        raise AssertionError(name+' multiple native focused controls')
                    return observed if len(observed)==1 else None
                neighbor=native.wait_for(neighbor_ready)
                result.setdefault('focus_round_trips',[]).append({'leave':leave,'return':back,'neighbor':neighbor})
                if (neighbor[0]['name'],neighbor[0]['role'])==(target['name'],target['role']):
                    continue
                quiet();marker=mark();key(back)
                current=native.wait_for(lambda:(f:=speech.focused(tree())) and len(f)==1
                    and f[0]['name'].startswith(label) and f[0])
                assert current['role']==target['role'],name+' role changed during focus round trip'
                announced(step,{'contains':expected},marker)
                return
            raise AssertionError(name+' native focus never left target: '+label)
        label=CASES[case][language=='fr']
        expected_focus=[label.rstrip(' ·…')]
        if case in ('tag-assignment','flag-bulk'):
            expected_focus.append('Mixte' if language=='fr' else 'Mixed')
        announce_focus(name+'-focus',label,expected_focus)
        snapshot('focused')
        before=state()
        if case=='browse':
            key('ctrl+a');key('BackSpace')
            native.wait_for(lambda:(s:=alive()) and s['query']=='')
            desktop.french_dead_keys()
            native.wait_for(lambda: (s:=alive()) and s['query']=='êé az' and s)
            result['utf8_native_keys']=True
        elif case=='details':
            assert not before['technical_open'],'technical disclosure was already open'
            key('space');native.wait_for(lambda:(s:=alive()) and s['technical_open']
                and 'Bibliothèque originale.plain.pk' in s['technical'])
        elif case in ('tag-manager','collections'):
            key('ctrl+a');desktop.text('Été à garder')
            native.wait_for(lambda:any(n.get('text')=='Été à garder' and n['states']['focused'] for n in tree()))
        elif case=='tag-assignment':
            assert before['assignment_requested']==2 and before['bedtime_assigned']==1,'mixed assignment missing'
            key('Up')
            native.wait_for(lambda:(s:=alive()) and s['revision']>before['revision'] and s['bedtime_assigned']==2
                and s['assignment_result']=={'requested':2,'changed':1,'added':1,'removed':0})
            assert state()['selected_uuid']==before['selected_uuid'],'assignment changed inspection'
            announce_focus(name+'-assigned',label,['Bedtime','Attribuée' if language=='fr' else 'Assigned'])
        elif case=='flag-bulk':
            assert before['mixed'] and not before['broken'],'batch initial mixed state is false'
            key('Down');native.wait_for(lambda:(s:=alive()) and s['broken'] and s['undo'] and not s['flag_pending'])
            # Fresh focus must speak All, then actual Undo restores Mixed.
            announce_focus(name+'-all',label,[label.rstrip(' ·…'), 'Toutes défectueuses' if language=='fr' else 'All Broken'])
            focus('Annuler le changement de statut' if language=='fr' else 'Undo Broken change')
            quiet();marker=mark();key('space')
            native.wait_for(lambda:(s:=alive()) and s['mixed'] and not s['flag_pending'])
            announce_focus(name+'-mixed-restored',label,['Mixte' if language=='fr' else 'Mixed'])
            result['set_and_actual_undo']=True
        elif case=='shelf-members':
            order=before['member_order'];assert len(order)>1 and before['selected_uuid']==order[1]
            key('Down')
            focus('Exécuter' if language=='fr' else 'Run');key('space')
            expected=[order[1],order[0],*order[2:]]
            native.wait_for(lambda:(s:=alive()) and s['revision']>before['revision'] and s['member_order']==expected
                and s['selected_uuid']==before['selected_uuid'])
            # Moving to the first position correctly disables Run/Move up.
            # The mounted action chooser remains an enabled focus target.
            announce_focus(name+'-after-reorder',label,[label])
        elif case=='export':
            assert before['export_start'];key('space')
            native.wait_for(lambda:(s:=alive()) and s['dialog']==19)
        elif case=='setup':
            assert before['phase']=='Setup';key('space')
            native.wait_for(lambda:(s:=alive()) and s['phase']=='Library')
        elif case=='relocation':
            assert before['settings_proposed'] and before['settings_current']!=before['settings_proposed']
            key('space');native.wait_for(lambda:(s:=alive()) and s['revision']>before['revision'] and not s['settings_busy'])
            native.wait_for(lambda:(s:=alive()) and s['phase']=='Library' and s['settings_current']==before['settings_proposed']
                and s['settings_activation']>before['settings_activation'] and s['settings_result'] in ('Moved','CleanupPending')
                and s['settings_error'] is None)
        elif case=='schema':
            assert before['dialog']==22 and before['schema'];key('Escape')
            after=native.wait_for(lambda:(s:=alive()) and s['dialog']==5 and s)
            assert after['revision']==before['revision'] and after['phase']=='Recovery' and after['schema']
            expected='Conserver la base étrangère' if language=='fr' else 'Preserve foreign database'
            native.wait_for(lambda:(f:=speech.focused(tree())) and len(f)==1 and f[0]['name'].startswith(expected))
        elif case=='review-delete':
            assert before['review_shown'];key('space')
            native.wait_for(lambda:(s:=alive()) and not s['review_shown'])
        elif case in ('removal','tag-delete','collection-impact','shelf-create'):
            key('space');native.wait_for(lambda:(s:=alive()) and s['dialog']!=before['dialog'])
        elif case in ('filters','collection-union','settings'):
            assert before['dialog']=={'filters':8,'collection-union':13,'settings':2}[case]
            key('Escape');native.wait_for(lambda:(s:=alive()) and s['dialog']==0 and s['focused_key']==30)
            expected='Rechercher des histoires' if language=='fr' else 'Search stories'
            native.wait_for(lambda:(f:=speech.focused(tree())) and len(f)==1 and f[0]['name'].startswith(expected))
        else:
            key('space');native.wait_for(lambda:(s:=alive()) and s['revision']>before['revision'])
        native.wait_for(lambda:(s:=alive()) and s['revision']==s['offered_revision'])
        snapshot('after-keyboard-action')
        result['passed']=True
    except Exception:
        result['error']=traceback.format_exc()
        try:snapshot('failed')
        except Exception:result['failure_snapshot_error']=traceback.format_exc()
        raise
    finally:
        # Typed fixture-only shutdown asks the real worker to stop and join.
        (output/'finish.tmp').write_text('workflow-finish-v1\n')
        (output/'finish.tmp').replace(output/'finish')
        app.wait(timeout=20)
        assert app.returncode==0,name+' failed native shutdown'
        result['closed']=json.loads((output/'closed.json').read_text())
        assert result['closed']=={'actual_worker_joined':True}

def main():
    parser=argparse.ArgumentParser();parser.add_argument('--session',choices=['x11','wayland'],required=True)
    parser.add_argument('--binary',type=Path,required=True);parser.add_argument('--output',type=Path,required=True)
    parser.add_argument('--case',choices=list(CASES),required=True)
    parser.add_argument('--language',choices=['en','fr'],required=True);args=parser.parse_args()
    root=args.output;root.mkdir(exist_ok=True,parents=True)
    assert (root/'.workflow-speech-fixture').read_text()=='workflow-speech-v1\n'
    assert args.binary.is_absolute() and args.binary.is_file()
    report={'method':'native-workflows-orca-pcm-v5','session':args.session,'binary_sha256':digest(args.binary),'cases':[],'passed':False}
    desktop=None;pcm=None
    try:
        desktop=native.Desktop(args.session,root)
        report['application_environment']={k:os.environ.get(k) for k in ('DISPLAY','WAYLAND_DISPLAY','GDK_BACKEND','SLINT_BACKEND')}
        mark,announced,pcm,audio=speech.start(desktop,report)
        print(args.case,args.language,flush=True)
        run_case(desktop,args.binary,args.case,args.language,mark,announced,pcm,report)
        assert not desktop.overflow,desktop.overflow
        assert (root/'orca-debug.txt').stat().st_size<speech.evidence_limit(root/'orca-debug.txt')
        report['passed']=True
    except Exception:
        report['error']=traceback.format_exc();raise
    finally:
        if desktop:desktop.close();report['overflow']=desktop.overflow
        if pcm and pcm.exists():
            with wave.open(str(root/'speech.wav'),'wb') as output:
                output.setnchannels(1);output.setsampwidth(2);output.setframerate(16000);output.writeframes(pcm.read_bytes())
        report['files']={str(p.relative_to(root)):{'bytes':p.stat().st_size,'sha256':digest(p)} for p in root.rglob('*') if p.is_file() and p.name!='report.json'}
        (root/'report.json').write_text(json.dumps(report,ensure_ascii=False,indent=2)+'\n')
if __name__=='__main__':main()
