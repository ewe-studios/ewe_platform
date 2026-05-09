if (Test-Path 'C:\Users\vagrant\project\Cargo.toml') { 'mounted' }
elseif (Test-Path 'Z:\Cargo.toml') { 'mounted' }
else { 'missing' }
