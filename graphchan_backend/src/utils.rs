//! Shared helpers and constants will live here.

use chrono::Utc;

pub const APP_NAME: &str = "graphchan_backend";

pub fn now_utc_iso() -> String {
    Utc::now().to_rfc3339()
}

/// Take the first `n` characters of a string for display, char-aware (won't panic on multibyte
/// or short strings). Used for "Unknown (abc123…)" stub usernames where peer IDs may not be
/// hex / fixed length.
pub fn short_id(s: &str, n: usize) -> String {
    s.chars().take(n).collect()
}

pub fn print_banner() {
    println!(
        r#"
                            ++                                                              
                             ++                               ##                
                          ++  +++                            ##                 
                            ++ +++                          ##                  
                              ++++                         ###                  
                       +++++    ++                        ##                    
                       ++###++   ++                     +#+                     
                      +++####++  ##+                  +##                       
                      +++++####+ +#+               ++##                         
                      +++++#####+++             ++#+                            
                       +#+++++##+++        ++++##+                              
                        +########+#  +++++###                                   
                         +++#+#####+##+                 ORBWEAVER ACTIVATED                        
##                        ++##########+                                         
 ###+                   ++##++##+###+++##++                                     
   +##+              +++++  +###+##+ +++ ###++                                  
      ##++     ++++##+#    +++##  ###       ####++                              
         #######++         +# +#                +####                           
                           +#  #+                    #####                      
                          +#   +#+                        ####+                 
                          #     ++                            ##########        
                         ++     ###                                     ########
                        +++       ##                                            
                         +         ##                                           
                                    ###                                         
                                      ####+++#########                          
"#
    );
}
