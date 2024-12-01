import hashlib
import json
import os
import shutil

from typing import Dict, List, Set, Tuple
from util import make_data_dir, get_data_dir

def get_ietf_collections(docs: List[dict], modules: List[dict]) -> List[dict]:
    # set that contains all docs that are updates to other docs
    update_docs: Set[str] = set()
    for doc in docs:
        for updated_by in doc['updated_by']:
            update_docs.add(updated_by)

    # list that contains all docs that are not updates to other docs ("root docs")
    root_docs: List[dict] = []
    for doc in docs:
        if doc['doc_id'] not in update_docs:
            root_docs.append(doc)

    # list containing tuple of (each root doc, each update that doc has)
    docs_with_updates: List[Tuple[dict, List[dict]]] = []
    for root_doc in root_docs:
        update_ids = root_doc['updated_by']
        if len(update_ids) == 0:
            updates = []
        else:
            updates = list(map(lambda update_id: next(doc for doc in docs if doc['doc_id'] == update_id), update_ids))
        docs_with_updates.append((root_doc, updates))

    collections = []
    for (root_doc, update_ids) in docs_with_updates:
        doc_modules = set()
        update_modules = []

        # make updates a set as an optimization
        updates_set = set(map(lambda update: update['doc_id'], update_ids))

        for module in modules:
            file_path = f'./modules/{module["module_name"]}.asn'
            module_doc = module['doc_id']
            if module_doc == root_doc['doc_id']:
                doc_modules.add(file_path)
            elif module_doc in updates_set:
                update_doc = None
                for update_module in update_modules:
                    if update_module['doc'] == module_doc:
                        update_doc = update_module
                        break

                if update_doc is None:
                    doc = next(update for update in update_ids if update['doc_id'] == module_doc)
                    update_doc = {
                        'doc': doc['doc_id'],
                        'title': doc['title'],
                        'url': doc['url'],
                        'modules': set(),
                    }
                    update_modules.append(update_doc)

                update_doc['modules'].add(file_path)

        if len(doc_modules) > 0:
            collection = {
                'doc': root_doc['doc_id'],
                'title': root_doc['title'],
                'url': root_doc['url'],
                'modules': list(doc_modules),
            }
            if len(update_modules) > 0:
                for update_module in update_modules:
                    update_module['modules'] = list(update_module['modules'])
                collection['updates'] = update_modules
            collections.append(collection)

    return collections

def create_ietf_registry(registry_dir: str) -> bytes:
    ietf_data_dir = get_data_dir('IETF')
    with open(os.path.join(ietf_data_dir, 'index.json'), 'r') as f:
        index = json.load(f)
    with open(os.path.join(ietf_data_dir, 'downloads.json'), 'r') as f:
        downloads = json.load(f)

    shutil.copytree(os.path.join(ietf_data_dir, 'modules'), os.path.join(registry_dir, 'modules'), dirs_exist_ok=True)

    collections = get_ietf_collections(index, downloads['modules'])
    collections_json = json.dumps(collections, sort_keys=True)
    with open(os.path.join(registry_dir, 'registry.json'), 'w') as f:
        f.write(collections_json)
        hash = hashlib.sha256()
        hash.update(collections_json.encode())
        return hash.digest()

def get_itu_t_collections(recs: List[dict], modules: List[dict]) -> List[dict]:
    # fast lookup for recommendations
    rec_lookup: Dict[str, dict] = {}
    for rec in recs:
        rec_lookup[rec['name']] = rec

    # mapping of each rec name -> the modules in that rec
    modules_by_rec: Dict[str, List[dict]] = {}
    for module in modules:
        rec = rec_lookup[module['rec']]
        rec_name = rec['name']
        if rec_name not in modules_by_rec:
            modules_by_rec[rec_name] = []

        rec_modules = modules_by_rec[rec_name]
        rec_modules.append(module)

    collections = []
    for (rec_name, modules) in modules_by_rec.items():
        rec = rec_lookup[rec_name]
        collections.append({
            'name': rec['name'],
            'approval': rec['approval'],
            'title': rec['title'].replace('\u2013', '-'),
            'modules': list(map(lambda module: f'./modules/{rec["name"]}/{module["name"]}.asn', modules)),
        })

    return collections

def create_itu_t_registry(registry_dir: str) -> bytes:
    itu_t_data_dir = get_data_dir('ITU-T')
    with open(os.path.join(itu_t_data_dir, 'recommendations.json'), 'r') as f:
        recommendations = json.load(f)
    with open(os.path.join(itu_t_data_dir, 'modules.json'), 'r') as f:
        modules = json.load(f)

    shutil.copytree(os.path.join(itu_t_data_dir, 'modules'), os.path.join(registry_dir, 'modules'), dirs_exist_ok=True)

    collections = get_itu_t_collections(recommendations, modules)
    collections_json = json.dumps(collections, sort_keys=True)
    with open(os.path.join(registry_dir, 'registry.json'), 'w') as f:
        f.write(collections_json)
        hash = hashlib.sha256()
        hash.update(collections_json.encode())
        return hash.digest()


def main():
    data_dir = make_data_dir('package-registry')
    registry_dir = os.path.join(data_dir, 'registry')

    if not os.path.exists(registry_dir):
        os.mkdir(registry_dir)

    ietf_dir = os.path.join(registry_dir, 'IETF')
    if not os.path.exists(ietf_dir):
        os.mkdir(ietf_dir)
    ietf_hash = create_ietf_registry(ietf_dir)

    itu_t_dir = os.path.join(registry_dir, 'ITU-T')
    if not os.path.exists(itu_t_dir):
        os.mkdir(itu_t_dir)
    itu_t_hash = create_itu_t_registry(itu_t_dir)

    hash = hashlib.sha256()
    hash.update(ietf_hash)
    hash.update(itu_t_hash)
    registry_hash = hash.hexdigest()

    package_registry = {
        'hash': registry_hash,
        'orgs': [
            {
                'hash': ietf_hash.hex(),
                'name': 'IETF',
                'full_name': 'Internet Engineering Task Force',
                'desc': 'IETF is responsible for the technical standards (called RFCs) that make up the Internet protocol suite, such as PKIX for public-key infrastructure, PKCS for public-key cryptography, and many others.',
                'registry': './registry/IETF/registry.json',
            },
            {  
                'hash': itu_t_hash.hex(),
                'name': 'ITU-T',
                'full_name': 'International Telecommunication Union Telecommunication Standardization Sector',
                'desc': 'ITU-T is responsible for coordinating standards such as X.509 for cybersecurity, H.264 for video compression, and many others.',
                'registry': './registry/ITU-T/registry.json'
            }
        ],
    }

    with open(os.path.join(data_dir, 'hash.txt'), 'w') as f:
        f.write(registry_hash)

    with open(os.path.join(data_dir, 'package-registry.json'), 'w') as f:
        json.dump(package_registry, f)

if __name__ == '__main__':
    main()
