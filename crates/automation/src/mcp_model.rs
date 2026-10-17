//! Read-only service shared by the MCP transport and dependency-free tests.
use crate::{protocol::{Snapshot,SnapshotElement},Change,Graph,NodeKind,Property};
use openbim_document::Document;

pub struct SnapshotService { snapshot:Snapshot, document:Document }
pub struct ElementPage<'a> { pub items:Vec<&'a SnapshotElement>,pub total:usize,pub next_offset:Option<usize> }
pub struct ChangePage { pub changes:Vec<Change>,pub total:usize,pub next_offset:Option<usize> }
impl SnapshotService {
    pub fn new(mut snapshot:Snapshot)->Result<Self,String> {
        if snapshot.elements.len()>10_000 { return Err("MCP snapshot limit is 10,000 elements; export a smaller selection".into()); }
        snapshot.elements.sort_by_key(|e|e.id);
        let document=snapshot.materialize()?;
        Ok(Self { snapshot,document })
    }
    pub fn snapshot(&self)->&Snapshot { &self.snapshot }
    pub fn check_document(&self,id:&str)->Result<(),String> {
        if id!=self.snapshot.document_id { return Err("Document unavailable; call openbim_list_documents for the exported snapshot identity".into()); }
        Ok(())
    }
    pub fn query(&self,id:&str,category:Option<&str>,name:Option<&str>,offset:usize,limit:usize)->Result<ElementPage<'_>,String> {
        self.check_document(id)?; check_page(offset,limit)?;
        if let Some(category)=category {
            if !["Wall","Beam","Duct","Pipe","Equipment","Level","Grid"].contains(&category) { return Err("Category must be Wall, Beam, Duct, Pipe, Equipment, Level or Grid".into()); }
        }
        if name.is_some_and(|s|s.len()>256) { return Err("Name filter exceeds 256 bytes".into()); }
        let name=name.map(str::to_lowercase);
        let matches:Vec<_>=self.snapshot.elements.iter().filter(|e|
            category.is_none_or(|c|e.category==c) && name.as_ref().is_none_or(|n|e.name.to_lowercase().contains(n))
        ).collect();
        let total=matches.len();
        let items=matches.into_iter().skip(offset).take(limit).collect();
        Ok(ElementPage { items,total,next_offset:(offset.saturating_add(limit)<total).then_some(offset.saturating_add(limit)) })
    }
    pub fn element(&self,id:&str,element:u64)->Result<&SnapshotElement,String> {
        self.check_document(id)?;
        self.snapshot.elements.binary_search_by_key(&element,|e|e.id).ok().map(|index|&self.snapshot.elements[index])
            .ok_or_else(||"Element unavailable in this snapshot; use openbim_query_elements to find an ID".into())
    }
    pub fn validate(&self,graph:&Graph)->Result<Vec<u64>,String> {
        if graph.nodes.len()>1000 || graph.wires.len()>3000 { return Err("MCP graph limit is 1,000 nodes and 3,000 wires".into()); }
        if graph.nodes.iter().any(|n|matches!(&n.kind,NodeKind::Selection { ids } if ids.len()>10_000)) {
            return Err("MCP selection limit is 10,000 element IDs per node".into());
        }
        if graph.nodes.iter().any(|n|matches!(n.kind,NodeKind::Python { .. })) {
            return Err("Python nodes are disabled in this read-only MCP server; use native nodes".into());
        }
        graph.validate().map_err(|e|format!("Invalid graph: {e:?}; inspect openbim_get_node_catalog"))
    }
    pub fn build_property_graph(&self,id:&str,ids:Vec<u64>,property:Property,value:f64)->Result<Graph,String> {
        self.check_document(id)?;
        if ids.is_empty() || ids.len()>10_000 { return Err("Select between 1 and 10,000 element IDs".into()); }
        let mut graph=Graph::example(ids);
        graph.nodes[1].kind=NodeKind::Read { property };
        graph.nodes[2].kind=if property==Property::Slope { NodeKind::Ratio { value } } else { NodeKind::Length { metres:value } };
        graph.nodes[3].kind=NodeKind::Set { property };
        graph.preview(&self.document).map_err(|e|format!("Cannot build property graph: {e:?}"))?;
        Ok(graph)
    }
    pub fn preview(&self,id:&str,revision:u64,graph:&Graph,offset:usize,limit:usize)->Result<ChangePage,String> {
        self.check_document(id)?; check_page(offset,limit)?;
        if revision!=self.snapshot.revision { return Err(format!("Snapshot revision mismatch; expected {}. Re-query document context",self.snapshot.revision)); }
        self.validate(graph)?;
        let plan=graph.preview(&self.document).map_err(|e|format!("Preview rejected: {e:?}; correct the graph and retry"))?;
        let total=plan.changes().len();
        let changes=plan.changes().iter().skip(offset).take(limit).cloned().collect();
        Ok(ChangePage { changes,total,next_offset:(offset.saturating_add(limit)<total).then_some(offset.saturating_add(limit)) })
    }
}
fn check_page(offset:usize,limit:usize)->Result<(),String> {
    if limit==0 || limit>100 || offset>100_000 { return Err("Use limit 1..100 and offset 0..100000".into()); }
    Ok(())
}
